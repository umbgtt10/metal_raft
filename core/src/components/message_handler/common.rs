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

#[allow(clippy::type_complexity)]
pub fn send<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
    ctx.transport.send(to, msg);
}

#[allow(clippy::type_complexity)]
pub fn broadcast<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
    let mut ids = C::new();
    for peer in ctx.config_manager.config().peers(*ctx.id) {
        ids.push(peer).ok();
    }

    for peer in ids.iter() {
        send(ctx, peer, msg.clone());
    }
}

#[allow(clippy::type_complexity)]
pub fn node_state_to_role<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
    RoleTransitionManager::node_state_to_role(ctx.role)
}

#[allow(clippy::type_complexity)]
pub fn step_down<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
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

    *ctx.current_leader = None;
    ctx.leader_lease.revoke();
}

#[allow(clippy::type_complexity)]
pub fn validate_term_and_step_down<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
    if term > *ctx.current_term {
        step_down(ctx, term);
        true
    } else {
        false
    }
}

#[allow(clippy::type_complexity)]
pub fn reset_election_timer_if_valid_term<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
    if term >= *ctx.current_term {
        ctx.election.timer_service_mut().reset_election_timer();
    }
}

#[allow(clippy::type_complexity)]
pub fn apply_config_changes<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    CLK: Clock,
{
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

    let new_role = node_state_to_role(ctx);
    if old_role != new_role {
        ctx.observer
            .role_changed(*ctx.id, old_role, new_role, *ctx.current_term);
    }
}
