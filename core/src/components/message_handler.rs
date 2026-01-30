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
        config_change_manager::ConfigError, message_handler_context::MessageHandlerContext,
    },
    log_entry::ConfigurationChange,
    observer::Observer,
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::{TimerKind, TimerService},
    transport::Transport,
    types::{LogIndex, NodeId},
};

mod admin;
mod common;
mod election;
mod replication;

type PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK> =
    core::marker::PhantomData<(T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    NotLeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Node is not the leader or does not have a valid lease
    NotLeaderOrNoLease,
}

/// MessageHandler handles all Raft message processing.
/// This is a zero-sized stateless type that separates message handling logic from RaftNode.
pub struct MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
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
    _phantom: PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
    MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
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
    pub fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    /// Main entry point for handling messages
    #[allow(clippy::type_complexity)]
    pub fn handle_message(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        from: NodeId,
        msg: RaftMsg<P, L, CC>,
    ) where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        match msg {
            RaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => election::handle_pre_vote_request(
                ctx,
                from,
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            ),

            RaftMsg::PreVoteResponse { term, vote_granted } => {
                election::handle_pre_vote_response(ctx, from, term, vote_granted)
            }

            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => election::handle_vote_request(
                ctx,
                from,
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            ),

            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                election::handle_vote_response(ctx, from, term, vote_granted)
            }

            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => replication::handle_append_entries(
                ctx,
                from,
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            ),

            RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => replication::handle_append_entries_response(ctx, from, term, success, match_index),

            RaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            } => replication::handle_install_snapshot(
                ctx,
                from,
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            ),

            RaftMsg::InstallSnapshotResponse { term, success } => {
                replication::handle_install_snapshot_response(ctx, from, term, success)
            }
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn handle_timer(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        kind: TimerKind,
    ) {
        match kind {
            TimerKind::Election => {
                election::handle_election_timer(ctx);
            }
            TimerKind::Heartbeat => {
                replication::handle_heartbeat_timer(ctx);
            }
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn submit_client_command(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        payload: P,
    ) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        admin::submit_client_command(ctx, payload)
    }

    #[allow(clippy::type_complexity)]
    pub fn submit_config_change(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        change: ConfigurationChange,
    ) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        admin::submit_config_change(ctx, change)
    }

    #[allow(clippy::type_complexity)]
    pub fn start_pre_vote(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    ) {
        election::start_pre_vote(ctx);
    }

    #[allow(clippy::type_complexity)]
    pub fn start_election(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    ) {
        election::start_election(ctx);
    }

    /// Add a server to the cluster configuration
    #[allow(clippy::type_complexity)]
    pub fn add_server(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        node_id: NodeId,
    ) -> Result<LogIndex, ConfigError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
        C: NodeCollection,
    {
        admin::add_server(ctx, node_id)
    }

    /// Remove a server from the cluster configuration
    #[allow(clippy::type_complexity)]
    pub fn remove_server(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        node_id: NodeId,
    ) -> Result<LogIndex, ConfigError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
        C: NodeCollection,
    {
        admin::remove_server(ctx, node_id)
    }

    #[allow(clippy::type_complexity)]
    pub fn send(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        to: NodeId,
        msg: RaftMsg<P, L, CC>,
    ) {
        common::send(ctx, to, msg);
    }

    #[allow(clippy::type_complexity)]
    pub fn broadcast(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        msg: RaftMsg<P, L, CC>,
    ) {
        common::broadcast(ctx, msg);
    }

    #[allow(clippy::type_complexity)]
    pub fn send_heartbeats(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    ) {
        replication::send_heartbeats(ctx);
    }

    #[allow(clippy::type_complexity)]
    pub fn send_append_entries_to_followers(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
    ) {
        replication::send_append_entries_to_followers(ctx);
    }

    #[allow(clippy::type_complexity)]
    pub fn apply_config_changes(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>,
        changes: CCC,
    ) {
        common::apply_config_changes(ctx, changes);
    }
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK> Default
    for MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC, CLK>
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
    fn default() -> Self {
        Self::new()
    }
}
