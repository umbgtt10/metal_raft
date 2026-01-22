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
    components::message_handler::{common, MessageHandlerContext},
    node_state::NodeState,
    observer::{Observer, TimerKind as ObserverTimerKind},
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn handle_append_entries<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    from: NodeId,
    term: Term,
    prev_log_index: LogIndex,
    prev_log_term: Term,
    entries: L,
    leader_commit: LogIndex,
) where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
    CLK: Clock,
{
    if common::validate_term_and_step_down(ctx, term) {
        return;
    }

    common::reset_election_timer_if_valid_term(ctx, term);

    let (response, config_changes) = ctx.replication.handle_append_entries(
        term,
        prev_log_index,
        prev_log_term,
        entries,
        leader_commit,
        ctx.current_term,
        ctx.storage,
        ctx.state_machine,
        ctx.role,
    );
    // Note: apply_config_changes logic is in admin.rs. But we need it here?
    // This is a circular dependency. Admin -> Common?
    // apply_config_changes calls config_manager.apply_changes.
    // It's simple enough to be in common or duplicated or in replication?
    // Let's put apply_config_changes in common.rs!

    // For now, assuming it's in common or handled here.
    // If apply_config_changes is in common, we are good.
    // Let's assume common::apply_config_changes exists.
    common::apply_config_changes(ctx, config_changes);
    common::send(ctx, from, response);
}

#[allow(clippy::type_complexity)]
pub fn handle_append_entries_response<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    from: NodeId,
    term: Term,
    success: bool,
    match_index: LogIndex,
) where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
    CLK: Clock,
{
    if common::validate_term_and_step_down(ctx, term) {
        return;
    }

    if *ctx.role == NodeState::Leader && term == *ctx.current_term {
        let old_commit_index = ctx.replication.commit_index();
        let config_changes: CCC = ctx.replication.handle_append_entries_response(
            from,
            success,
            match_index,
            ctx.storage,
            ctx.state_machine,
            ctx.config_manager.config(),
            *ctx.id,
        );
        let new_commit_index = ctx.replication.commit_index();

        if new_commit_index > old_commit_index {
            ctx.observer
                .commit_advanced(*ctx.id, old_commit_index, new_commit_index);

            // Grant leader lease when commit advances (quorum acknowledgment)
            ctx.leader_lease.grant();

            // Apply any configuration changes
            common::apply_config_changes(ctx, config_changes);

            // Check if we should create a snapshot after commit advances
            if should_create_snapshot(ctx) {
                let _ = create_snapshot_internal(ctx);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn handle_install_snapshot<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    from: NodeId,
    term: Term,
    leader_id: NodeId,
    last_included_index: LogIndex,
    last_included_term: Term,
    offset: u64,
    data: CC,
    done: bool,
) where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
    CLK: Clock,
{
    common::reset_election_timer_if_valid_term(ctx, term);

    let response = ctx.replication.handle_install_snapshot(
        term,
        leader_id,
        last_included_index,
        last_included_term,
        offset,
        data,
        done,
        ctx.current_term,
        ctx.storage,
        ctx.state_machine,
        ctx.role,
    );
    common::send(ctx, from, response);
}

#[allow(clippy::type_complexity)]
pub fn handle_install_snapshot_response<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    from: NodeId,
    term: Term,
    success: bool,
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
    if common::validate_term_and_step_down(ctx, term) {
        return;
    }

    if *ctx.role == NodeState::Leader && term == *ctx.current_term {
        // Get the snapshot metadata to know last_included_index
        if let Some(snapshot_metadata) = ctx.storage.snapshot_metadata() {
            ctx.replication.handle_install_snapshot_response(
                from,
                term,
                success,
                snapshot_metadata.last_included_index,
            );
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn send_heartbeats<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    send_append_entries_to_followers(ctx);
    ctx.election.timer_service_mut().reset_heartbeat_timer();
}

#[allow(clippy::type_complexity)]
pub fn send_append_entries_to_followers<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    // Collect peer IDs first to avoid borrowing issues (excluding self)
    let mut ids = C::new();
    for peer in ctx.config_manager.config().peers(*ctx.id) {
        ids.push(peer).ok();
    }

    // Now send to each peer
    for peer in ids.iter() {
        let msg = ctx
            .replication
            .get_append_entries_for_peer(peer, *ctx.id, ctx.storage);
        common::send(ctx, peer, msg);
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_heartbeat_timer<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    ctx.observer
        .timer_fired(*ctx.id, ObserverTimerKind::Heartbeat, *ctx.current_term);

    if *ctx.role == NodeState::Leader {
        send_heartbeats(ctx);
    }
}

#[allow(clippy::type_complexity)]
fn should_create_snapshot<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
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
    ctx.snapshot_manager
        .should_create(ctx.replication.commit_index(), ctx.storage)
}

#[allow(clippy::type_complexity)]
fn create_snapshot_internal<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
) -> Result<(), ()>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
    CLK: Clock,
{
    let commit_index = ctx.replication.commit_index();
    ctx.snapshot_manager
        .create(ctx.storage, ctx.state_machine, commit_index)
        .map(|_| ())
        .map_err(|_| ())
}
