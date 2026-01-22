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
        config_change_manager::ConfigError,
        message_handler::{common, replication, ClientError, MessageHandlerContext},
    },
    log_entry::{ConfigurationChange, EntryType, LogEntry},
    node_state::NodeState,
    observer::Observer,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId},
};

/// Add a server to the cluster configuration
#[allow(clippy::type_complexity)]
pub fn add_server<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    node_id: NodeId,
) -> Result<LogIndex, ConfigError>
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
    let is_leader = *ctx.role == NodeState::Leader;
    let commit_index = ctx.replication.commit_index();

    // Validate and get the configuration change
    let change = ctx
        .config_manager
        .add_server(node_id, *ctx.id, is_leader, commit_index)?;

    // Submit the change
    let index = submit_config_change(ctx, change).map_err(|_| ConfigError::NotLeader)?;

    // Track the pending change
    ctx.config_manager.track_pending_change(index);

    Ok(index)
}

/// Remove a server from the cluster configuration
#[allow(clippy::type_complexity)]
pub fn remove_server<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    node_id: NodeId,
) -> Result<LogIndex, ConfigError>
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
    let is_leader = *ctx.role == NodeState::Leader;
    let commit_index = ctx.replication.commit_index();

    // Validate and get the configuration change
    let change = ctx
        .config_manager
        .remove_server(node_id, *ctx.id, is_leader, commit_index)?;

    // Submit the change
    let index = submit_config_change(ctx, change).map_err(|_| ConfigError::NotLeader)?;

    // Track the pending change
    ctx.config_manager.track_pending_change(index);

    Ok(index)
}

#[allow(clippy::type_complexity)]
pub fn submit_client_command<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    payload: P,
) -> Result<LogIndex, ClientError>
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
    if *ctx.role != NodeState::Leader {
        return Err(ClientError::NotLeader);
    }

    let entry = LogEntry {
        term: *ctx.current_term,
        entry_type: EntryType::Command(payload),
    };
    ctx.storage.append_entries(&[entry]);
    let index = ctx.storage.last_log_index();

    // If we are a single node cluster (only member is self) or empty config, advance commit immediately
    if ctx.config_manager.config().members.len() <= 1 {
        let config_changes: CCC = ctx.replication.advance_commit_index(
            ctx.storage,
            ctx.state_machine,
            ctx.config_manager.config(),
            *ctx.id,
        );
        common::apply_config_changes(ctx, config_changes);
    }

    replication::send_append_entries_to_followers(ctx);

    Ok(index)
}

#[allow(clippy::type_complexity)]
pub fn submit_config_change<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>(
    ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    change: ConfigurationChange,
) -> Result<LogIndex, ClientError>
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
    if *ctx.role != NodeState::Leader {
        return Err(ClientError::NotLeader);
    }

    let entry = LogEntry {
        term: *ctx.current_term,
        entry_type: EntryType::ConfigChange(change),
    };
    ctx.storage.append_entries(&[entry]);
    let index = ctx.storage.last_log_index();

    // If we are a single node cluster (only member is self) or empty config, advance commit immediately
    if ctx.config_manager.config().members.len() <= 1 {
        let config_changes: CCC = ctx.replication.advance_commit_index(
            ctx.storage,
            ctx.state_machine,
            ctx.config_manager.config(),
            *ctx.id,
        );
        common::apply_config_changes(ctx, config_changes);
    }

    replication::send_append_entries_to_followers(ctx);

    Ok(index)
}
