// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection},
    types::{LogIndex, NodeId, Term},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventLevel {
    None = 0,
    Essential = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

/// Timer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    Election,
    Heartbeat,
}

pub trait Observer {
    type Payload: Clone;

    type LogEntries: LogEntryCollection<Payload = Self::Payload> + Clone;
    type ChunkCollection: ChunkCollection + Clone;

    fn min_level(&self) -> EventLevel;

    // === Leadership Events (Essential) ===

    fn leader_elected(&mut self, node: NodeId, term: Term);
    fn role_changed(&mut self, node: NodeId, old: Role, new: Role, term: Term);
    fn leader_lost(&mut self, node: NodeId, old_term: Term, new_term: Term);

    // === Election Events (Info) ===
    fn election_started(&mut self, node: NodeId, term: Term);
    fn election_timeout(&mut self, node: NodeId, term: Term);
    fn pre_vote_started(&mut self, node: NodeId, term: Term);
    fn pre_vote_requested(
        &mut self,
        candidate: NodeId,
        voter: NodeId,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
    );
    fn pre_vote_granted(&mut self, candidate: NodeId, voter: NodeId, granted: bool, term: Term);
    fn pre_vote_succeeded(&mut self, node: NodeId, term: Term);

    // === Log Events (Info) ===
    fn commit_advanced(&mut self, node: NodeId, old_index: LogIndex, new_index: LogIndex);
    fn log_appended(&mut self, node: NodeId, count: usize, last_index: LogIndex, last_term: Term);
    fn log_truncated(&mut self, node: NodeId, from_index: LogIndex);

    // === Voting Events (Debug) ===
    fn vote_requested(
        &mut self,
        candidate: NodeId,
        voter: NodeId,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
    );
    fn vote_granted(&mut self, candidate: NodeId, voter: NodeId, granted: bool, term: Term);
    fn voted_for(&mut self, voter: NodeId, candidate: NodeId, term: Term);

    // === Replication Events (Debug) ===
    fn heartbeat_sent(&mut self, leader: NodeId, follower: NodeId, term: Term);
    fn append_entries_sent(
        &mut self,
        leader: NodeId,
        follower: NodeId,
        entry_count: usize,
        term: Term,
    );
    fn append_entries_response(
        &mut self,
        follower: NodeId,
        leader: NodeId,
        success: bool,
        match_index: LogIndex,
        term: Term,
    );

    // === Timer Events (Debug) ===
    fn timer_fired(&mut self, node: NodeId, timer: TimerKind, term: Term);
    fn timer_reset(&mut self, node: NodeId, timer: TimerKind, term: Term);

    // === Message Events (Trace) ===
    fn message_sent(
        &mut self,
        from: NodeId,
        to: NodeId,
        msg: &crate::raft_messages::RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    );
    fn message_received(
        &mut self,
        to: NodeId,
        from: NodeId,
        msg: &crate::raft_messages::RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    );
    fn higher_term_discovered(
        &mut self,
        node: NodeId,
        old_term: Term,
        new_term: Term,
        from_peer: NodeId,
    );
    fn configuration_change_applied(&mut self, node: NodeId, target: NodeId, added: bool);

    // === State Machine Events (Debug) ===
    fn state_machine_applied(&mut self, node: NodeId, index: LogIndex, payload: &Self::Payload);
    fn state_machine_read(&mut self, node: NodeId, key: &str, found: bool);
}
