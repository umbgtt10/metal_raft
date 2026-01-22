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
        config_change_manager::{ConfigChangeManager, ConfigError},
        election_manager::ElectionManager,
        log_replication_manager::LogReplicationManager,
        snapshot_manager::SnapshotManager,
    },
    log_entry::ConfigurationChange,
    node_state::NodeState,
    observer::Observer,
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

mod admin;
mod common;
mod election;
mod replication;

type PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC> =
    core::marker::PhantomData<(T, S, P, SM, C, L, CC, M, TS, O, CCC)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    NotLeader,
}

/// MessageHandler handles all Raft message processing.
/// This is a zero-sized stateless type that separates message handling logic from RaftNode.
pub struct MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    _phantom: PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
}

/// Context struct that bundles all mutable references needed by MessageHandler
pub struct MessageHandlerContext<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    pub _phantom: core::marker::PhantomData<CCC>,
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    pub fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    /// Main entry point for handling messages
    pub fn handle_message(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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

    pub fn handle_timer(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        kind: crate::timer_service::TimerKind,
    ) {
        match kind {
            crate::timer_service::TimerKind::Election => {
                election::handle_election_timer(ctx);
            }
            crate::timer_service::TimerKind::Heartbeat => {
                replication::handle_heartbeat_timer(ctx);
            }
        }
    }

    pub fn submit_client_command(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        payload: P,
    ) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        admin::submit_client_command(ctx, payload)
    }

    pub fn submit_config_change(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        change: ConfigurationChange,
    ) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        admin::submit_config_change(ctx, change)
    }

    pub fn start_pre_vote(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        election::start_pre_vote(ctx);
    }

    pub fn start_election(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        election::start_election(ctx);
    }

    /// Add a server to the cluster configuration
    pub fn add_server(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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
    pub fn remove_server(
        &self,
        ctx: &mut MessageHandlerContext<'_, T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        node_id: NodeId,
    ) -> Result<LogIndex, ConfigError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
        C: NodeCollection,
    {
        admin::remove_server(ctx, node_id)
    }

    pub fn send(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        to: NodeId,
        msg: RaftMsg<P, L, CC>,
    ) {
        common::send(ctx, to, msg);
    }

    pub fn broadcast(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        msg: RaftMsg<P, L, CC>,
    ) {
        common::broadcast(ctx, msg);
    }

    pub fn send_heartbeats(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        replication::send_heartbeats(ctx);
    }

    pub fn send_append_entries_to_followers(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        replication::send_append_entries_to_followers(ctx);
    }

    pub fn apply_config_changes(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        changes: CCC,
    ) {
        common::apply_config_changes(ctx, changes);
    }
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> Default
    for MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    fn default() -> Self {
        Self::new()
    }
}
