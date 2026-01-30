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
        config_change_manager::ConfigChangeManager, election_manager::ElectionManager,
        leader_lease::LeaderLease, log_replication_manager::LogReplicationManager,
        snapshot_manager::SnapshotManager,
    },
    node_state::NodeState,
    observer::Observer,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{NodeId, Term},
};

pub struct MessageHandlerContext<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
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
    CLK: Clock,
{
    pub id: &'a NodeId,
    pub role: &'a mut NodeState,
    pub current_term: &'a mut Term,
    pub transport: &'a mut T,
    pub storage: &'a mut S,
    pub state_machine: &'a mut SM,
    pub observer: &'a mut O,
    pub election: &'a mut ElectionManager<C, TS>,
    pub replication: &'a mut LogReplicationManager<M>,
    pub config_manager: &'a mut ConfigChangeManager<C, M>,
    pub snapshot_manager: &'a mut SnapshotManager,
    pub leader_lease: &'a mut LeaderLease<CLK>,
    pub _phantom: core::marker::PhantomData<CCC>,
}
