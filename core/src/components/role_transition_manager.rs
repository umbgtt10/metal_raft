// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    },
    node_state::NodeState,
    observer::{Observer, Role},
    raft_messages::RaftMsg,
    storage::Storage,
    timer_service::TimerService,
    types::{NodeId, Term},
};

/// Manages role transitions (Follower -> Candidate -> Leader) and related state changes
pub struct RoleTransitionManager;

impl RoleTransitionManager {
    pub fn new() -> Self {
        Self
    }

    /// Start a pre-vote phase to test if we can win an election
    pub fn start_pre_vote<P, L, CC, C, TS, O, S>(
        node_id: NodeId,
        current_term: Term,
        storage: &S,
        election: &mut ElectionManager<C, TS>,
        observer: &mut O,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        C: NodeCollection,
        TS: TimerService,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        observer.pre_vote_started(node_id, current_term);

        election.start_pre_vote(node_id, current_term, storage)
    }

    /// Start a real election (incrementing term and voting for self)
    pub fn start_election<P, L, CC, C, TS, O, S>(
        node_id: NodeId,
        current_term: &mut Term,
        storage: &mut S,
        role: &mut NodeState,
        election: &mut ElectionManager<C, TS>,
        observer: &mut O,
        old_role: Role,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        C: NodeCollection,
        TS: TimerService,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        let vote_request = election.start_election(node_id, current_term, storage, role);

        observer.election_started(node_id, *current_term);
        observer.role_changed(node_id, old_role, Role::Candidate, *current_term);
        observer.voted_for(node_id, node_id, *current_term);

        vote_request
    }

    #[allow(clippy::too_many_arguments)]
    pub fn become_leader<P, L, CC, C, M, TS, O, S>(
        node_id: NodeId,
        current_term: Term,
        role: &mut NodeState,
        storage: &S,
        members: impl Iterator<Item = NodeId>,
        election: &mut ElectionManager<C, TS>,
        replication: &mut LogReplicationManager<M>,
        observer: &mut O,
        old_role: Role,
    ) where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L>,
        L: LogEntryCollection<Payload = P>,
        C: NodeCollection,
        M: MapCollection,
        TS: TimerService,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        *role = NodeState::Leader;

        observer.role_changed(node_id, old_role, Role::Leader, current_term);
        observer.leader_elected(node_id, current_term);

        // Initialize replication state
        replication.initialize_leader_state(members, storage);

        election.timer_service_mut().stop_timers();
        election.timer_service_mut().reset_heartbeat_timer();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step_down<P, L, CC, C, TS, O, S>(
        node_id: NodeId,
        old_term: Term,
        new_term: Term,
        current_term: &mut Term,
        storage: &mut S,
        role: &mut NodeState,
        election: &mut ElectionManager<C, TS>,
        observer: &mut O,
        old_role: Role,
    ) where
        P: Clone,
        S: Storage<Payload = P>,
        L: LogEntryCollection<Payload = P>,
        C: NodeCollection,
        TS: TimerService,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        if old_role == Role::Leader {
            observer.leader_lost(node_id, old_term, new_term);
        }

        *current_term = new_term;
        storage.set_current_term(new_term);
        *role = NodeState::Follower;
        storage.set_voted_for(None);
        election.timer_service_mut().reset_election_timer();

        observer.role_changed(node_id, old_role, Role::Follower, new_term);
    }

    /// Convert NodeState to Observer Role
    pub fn node_state_to_role(state: &NodeState) -> Role {
        match state {
            NodeState::Follower => Role::Follower,
            NodeState::Candidate => Role::Candidate,
            NodeState::Leader => Role::Leader,
        }
    }
}

impl Default for RoleTransitionManager {
    fn default() -> Self {
        Self::new()
    }
}
