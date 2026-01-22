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
        message_handler::{common, replication, MessageHandlerContext},
        role_transition_manager::RoleTransitionManager,
    },
    node_state::NodeState,
    observer::{Observer, TimerKind as ObserverTimerKind},
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub fn handle_pre_vote_request<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    from: NodeId,
    term: Term,
    candidate_id: NodeId,
    last_log_index: LogIndex,
    last_log_term: Term,
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
    ctx.observer
        .pre_vote_requested(candidate_id, *ctx.id, term, last_log_index, last_log_term);

    let response = ctx.election.handle_pre_vote_request(
        term,
        candidate_id,
        last_log_index,
        last_log_term,
        *ctx.current_term,
        ctx.storage,
    );

    // Log the response
    if let RaftMsg::PreVoteResponse { vote_granted, .. } = &response {
        ctx.observer
            .pre_vote_granted(candidate_id, *ctx.id, *vote_granted, term);
    }

    common::send(ctx, from, response);
}

pub fn handle_pre_vote_response<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    from: NodeId,
    term: Term,
    vote_granted: bool,
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
    // Ignore pre-vote responses from higher terms
    if term > *ctx.current_term {
        return;
    }

    let should_start_election =
        ctx.election
            .handle_pre_vote_response(from, vote_granted, ctx.config_manager.config());

    if should_start_election {
        ctx.observer.pre_vote_succeeded(*ctx.id, *ctx.current_term);
        start_election(ctx);
    }
}

pub fn handle_vote_request<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    from: NodeId,
    term: Term,
    candidate_id: NodeId,
    last_log_index: LogIndex,
    last_log_term: Term,
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
    let response = ctx.election.handle_vote_request(
        term,
        candidate_id,
        last_log_index,
        last_log_term,
        ctx.current_term,
        ctx.storage,
        ctx.role,
    );
    common::send(ctx, from, response);
}

pub fn handle_vote_response<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    from: NodeId,
    term: Term,
    vote_granted: bool,
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
    if common::validate_term_and_step_down(ctx, term) {
        return;
    }

    let should_become_leader = ctx.election.handle_vote_response(
        from,
        term,
        vote_granted,
        ctx.current_term,
        ctx.role,
        ctx.config_manager.config(),
    );

    if should_become_leader {
        become_leader(ctx);
    }
}

pub fn start_pre_vote<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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
    let pre_vote_request = RoleTransitionManager::start_pre_vote(
        *ctx.id,
        *ctx.current_term,
        ctx.storage,
        ctx.election,
        ctx.observer,
    );

    common::broadcast(ctx, pre_vote_request);
}

pub fn start_election<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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

    let vote_request = RoleTransitionManager::start_election(
        *ctx.id,
        ctx.current_term,
        ctx.storage,
        ctx.role,
        ctx.election,
        ctx.observer,
        old_role,
    );

    common::broadcast(ctx, vote_request);

    // If we're the only member (no peers), we already have majority - become leader immediately
    if ctx.config_manager.config().members.len() <= 1 {
        become_leader(ctx);
    }
}

fn become_leader<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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

    RoleTransitionManager::become_leader(
        *ctx.id,
        *ctx.current_term,
        ctx.role,
        ctx.storage,
        ctx.config_manager.config().members.iter(),
        ctx.election,
        ctx.replication,
        ctx.observer,
        old_role,
    );

    // Send initial heartbeat
    replication::send_append_entries_to_followers(ctx);
}

pub fn handle_election_timer<T, S, P, SM, C, L, CC, M, TS, O, CCC>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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
    ctx.observer
        .timer_fired(*ctx.id, ObserverTimerKind::Election, *ctx.current_term);

    if *ctx.role != NodeState::Leader {
        ctx.observer.election_timeout(*ctx.id, *ctx.current_term);
        start_pre_vote(ctx);

        // If we have no peers (empty config or single-node), immediately start real election
        if ctx.config_manager.config().members.len() <= 1 {
            start_election(ctx);
        }
    }
}
