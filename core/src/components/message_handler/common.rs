// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        chunk_collection::ChunkCollection, config_change_collection::ConfigChangeCollection,
        log_entry_collection::LogEntryCollection, map_collection::MapCollection,
        node_collection::NodeCollection,
    },
    components::{
        message_handler::MessageHandlerContext, role_transition_manager::RoleTransitionManager,
    },
    observer::{Observer, Role},
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{NodeId, Term},
};

pub fn send<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    to: NodeId,
    msg: RaftMsg<P, L, CC>,
) where
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
    ctx.transport.send(to, msg);
}

pub fn broadcast<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    msg: RaftMsg<P, L, CC>,
) where
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
    // Collect peer IDs first to avoid borrowing issues
    let mut ids = C::new();
    for peer in ctx.config_manager.config().members.iter() {
        ids.push(peer).ok();
    }

    // Now send to each peer
    for peer in ids.iter() {
        send(ctx, peer, msg.clone());
    }
}

pub fn node_state_to_role<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
) -> Role
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
    RoleTransitionManager::node_state_to_role(ctx.role)
}

pub fn step_down<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    new_term: Term,
) where
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
    let old_role = RoleTransitionManager::node_state_to_role(ctx.role);
    let old_term = *ctx.current_term;

    RoleTransitionManager::step_down(
        *ctx.id,
        old_term,
        new_term,
        ctx.current_term,
        ctx.storage,
        ctx.role,
        ctx.election,
        ctx.observer,
        old_role,
    );
}

pub fn validate_term_and_step_down<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    term: Term,
) -> bool
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
    if term > *ctx.current_term {
        step_down(ctx, term);
        true
    } else {
        false
    }
}

pub fn reset_election_timer_if_valid_term<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    term: Term,
) where
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
    if term >= *ctx.current_term {
        ctx.election.timer_service_mut().reset_election_timer();
    }
}

pub fn apply_config_changes<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    changes: CCC,
) where
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
    // Check if we need to notify observer about role change (if we removed ourselves)
    let old_role = node_state_to_role(ctx);
    let last_log_index = ctx.storage.last_log_index();

    ctx.config_manager.apply_changes(
        changes,
        *ctx.id,
        last_log_index,
        ctx.replication,
        ctx.observer,
        ctx.role,
    );

    // If we stepped down due to self-removal, notify observer
    let new_role = node_state_to_role(ctx);
    if old_role != new_role {
        ctx.observer
            .role_changed(*ctx.id, old_role, new_role, *ctx.current_term);
    }
}
