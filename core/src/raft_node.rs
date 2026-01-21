// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        chunk_collection::ChunkCollection, config_change_collection::ConfigChangeCollection,
        configuration::Configuration, log_entry_collection::LogEntryCollection,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    components::{
        config_change_manager::ConfigChangeManager,
        election_manager::ElectionManager,
        log_replication_manager::LogReplicationManager,
        message_handler::{ClientError, MessageHandler, MessageHandlerContext},
        snapshot_manager::SnapshotManager,
    },
    event::Event,
    node_state::NodeState,
    observer::Observer,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub struct RaftNode<T, S, P, SM, C, L, CC, M, TS, O, CCC>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P>,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
{
    id: NodeId,
    role: NodeState,
    current_term: Term,
    transport: T,
    storage: S,
    state_machine: SM,
    observer: O,

    // Delegated responsibilities
    election: ElectionManager<C, TS>,
    replication: LogReplicationManager<M>,
    config_manager: ConfigChangeManager<C, M>,
    snapshot_manager: SnapshotManager,

    _phantom: core::marker::PhantomData<CCC>,
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> RaftNode<T, S, P, SM, C, L, CC, M, TS, O, CCC>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
{
    /// Internal constructor - use RaftNodeBuilder instead
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_from_builder(
        id: NodeId,
        storage: S,
        mut state_machine: SM,
        mut election: ElectionManager<C, TS>,
        mut replication: LogReplicationManager<M>,
        transport: T,
        peers: C,
        observer: O,
        snapshot_threshold: LogIndex,
    ) -> Self
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        let current_term = storage.current_term();

        // CRASH RECOVERY: Restore state machine from snapshot if one exists
        let mut last_applied = 0;
        if let Some(snapshot) = storage.load_snapshot() {
            // Restore state machine to snapshot state
            let _ = state_machine.restore_from_snapshot(&snapshot.data);
            last_applied = snapshot.metadata.last_included_index;

            // Note: Storage indices are already adjusted by load_snapshot
            // The storage implementation handles first_log_index internally
        }

        // NOTE: We do NOT replay uncommitted log entries on restart.
        // Raft safety requires that only committed entries are applied to the state machine.
        // After a crash, we don't know which entries were committed, so we only restore
        // from the snapshot. Uncommitted entries will be re-replicated by the leader.

        // Update replication manager's last_applied index
        replication.set_last_applied(last_applied);

        // Start election timer for initial Follower state
        election.timer_service_mut().reset_election_timer();

        let config_manager = ConfigChangeManager::new(Configuration::new(peers));
        let snapshot_manager = SnapshotManager::new(snapshot_threshold);

        RaftNode {
            id,
            role: NodeState::Follower,
            current_term,
            transport,
            storage,
            state_machine,
            observer,
            election,
            replication,
            config_manager,
            snapshot_manager,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn role(&self) -> &NodeState {
        &self.role
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn state_machine(&self) -> &SM {
        &self.state_machine
    }

    pub fn current_term(&self) -> Term {
        self.current_term
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn commit_index(&self) -> LogIndex {
        self.replication.commit_index()
    }

    pub fn peers(&self) -> Option<&C> {
        if self.config_manager.config().members.is_empty() {
            None
        } else {
            Some(&self.config_manager.config().members)
        }
    }

    pub fn timer_service(&self) -> &TS {
        self.election.timer_service()
    }

    pub fn config(&self) -> &Configuration<C> {
        self.config_manager.config()
    }

    pub fn is_committed(&self, index: LogIndex) -> bool {
        index <= self.replication.commit_index()
    }

    pub fn observer(&mut self) -> &mut O {
        &mut self.observer
    }

    pub fn on_event(&mut self, event: Event<P, L, CC>)
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        let handler = MessageHandler::new();
        let mut ctx = self.create_context();

        match event {
            Event::TimerFired(kind) => {
                handler.handle_timer(&mut ctx, kind);
            }
            Event::Message { from, msg } => {
                handler.handle_message(&mut ctx, from, msg);
            }
            Event::ClientCommand(payload) => {
                let _ = handler.submit_client_command(&mut ctx, payload);
            }
            Event::ConfigChange(change) => {
                let _ = handler.submit_config_change(&mut ctx, change);
            }
        }
    }

    pub fn submit_client_command(&mut self, payload: P) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        let handler = MessageHandler::new();
        let mut ctx = self.create_context();
        handler.submit_client_command(&mut ctx, payload)
    }

    fn create_context(
        &mut self,
    ) -> MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC> {
        MessageHandlerContext {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData,
        }
    }
}
