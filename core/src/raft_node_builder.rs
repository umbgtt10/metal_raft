// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    clock::Clock,
    collections::{
        chunk_collection::ChunkCollection, config_change_collection::ConfigChangeCollection,
        log_entry_collection::LogEntryCollection, map_collection::MapCollection,
        node_collection::NodeCollection,
    },
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    },
    observer::Observer,
    raft_node::RaftNode,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId},
};

pub struct RaftNodeBuilder<S, SM, P> {
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    lease_duration_millis: u64,
    _phantom: core::marker::PhantomData<P>,
}

impl<S, SM, P> RaftNodeBuilder<S, SM, P>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
{
    pub fn new(id: NodeId, storage: S, state_machine: SM) -> Self {
        Self {
            id,
            storage,
            state_machine,
            snapshot_threshold: 10,
            lease_duration_millis: 5000,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn with_snapshot_threshold(mut self, threshold: LogIndex) -> Self {
        self.snapshot_threshold = threshold;
        self
    }

    pub fn with_lease_duration(mut self, lease_duration_millis: u64) -> Self {
        self.lease_duration_millis = lease_duration_millis;
        self
    }

    pub fn with_election<C, TS>(
        self,
        election: ElectionManager<C, TS>,
    ) -> RaftNodeBuilderWithElection<S, SM, P, C, TS>
    where
        S: Storage<Payload = P>,
        SM: StateMachine<Payload = P>,
        P: Clone,
        C: NodeCollection,
        TS: TimerService,
    {
        RaftNodeBuilderWithElection {
            id: self.id,
            storage: self.storage,
            state_machine: self.state_machine,
            snapshot_threshold: self.snapshot_threshold,
            lease_duration_millis: self.lease_duration_millis,
            election,
        }
    }
}

pub struct RaftNodeBuilderWithElection<S, SM, P, C, TS>
where
    C: NodeCollection,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
{
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    lease_duration_millis: u64,
    election: ElectionManager<C, TS>,
}

impl<S, SM, P, C, TS> RaftNodeBuilderWithElection<S, SM, P, C, TS>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
{
    pub fn with_replication<M>(
        self,
        replication: LogReplicationManager<M>,
    ) -> RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
    where
        S: Storage<Payload = P>,
        SM: StateMachine<Payload = P>,
        P: Clone,
        C: NodeCollection,
        TS: TimerService,
        M: MapCollection,
    {
        RaftNodeBuilderWithReplication {
            id: self.id,
            storage: self.storage,
            state_machine: self.state_machine,
            snapshot_threshold: self.snapshot_threshold,
            election: self.election,
            replication,
            lease_duration_millis: self.lease_duration_millis,
        }
    }
}

pub struct RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
where
    C: NodeCollection,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
    M: MapCollection,
{
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    election: ElectionManager<C, TS>,
    replication: LogReplicationManager<M>,
    lease_duration_millis: u64,
}

impl<S, SM, P, C, TS, M> RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
    M: MapCollection,
{
    pub fn with_lease_duration(mut self, lease_duration_millis: u64) -> Self {
        self.lease_duration_millis = lease_duration_millis;
        self
    }

    pub fn with_transport<T, L, CC, O, CCC, CLK>(
        self,
        transport: T,
        peers: C,
        observer: O,
        clock: CLK,
    ) -> RaftNode<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
    where
        P: Clone,
        T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        CCC: ConfigChangeCollection,
        S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
        CLK: Clock,
    {
        RaftNode::new_from_builder(
            self.id,
            self.storage,
            self.state_machine,
            self.election,
            self.replication,
            transport,
            peers,
            observer,
            self.snapshot_threshold,
            clock,
            self.lease_duration_millis,
        )
    }
}
