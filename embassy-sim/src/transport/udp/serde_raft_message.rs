// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Serializable wire format for RaftMsg
//!
//! Since raft-core's RaftMsg doesn't derive Serialize/Deserialize,
//! we create a mirror type that can be serialized with postcard.

use crate::collections::embassy_log_collection::EmbassyLogEntryCollection;
use crate::collections::heapless_chunk_collection::HeaplessChunkVec;
use alloc::string::String;
use alloc::vec::Vec;
use raft_core::{
    collections::{chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection},
    log_entry::{ConfigurationChange, EntryType, LogEntry},
    raft_messages::RaftMsg,
};
use serde::{Deserialize, Serialize};

/// Serializable configuration change for wire protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireConfigurationChange {
    AddServer { node_id: u64 },
    RemoveServer { node_id: u64 },
}

impl From<ConfigurationChange> for WireConfigurationChange {
    fn from(change: ConfigurationChange) -> Self {
        match change {
            ConfigurationChange::AddServer(node_id) => {
                WireConfigurationChange::AddServer { node_id }
            }
            ConfigurationChange::RemoveServer(node_id) => {
                WireConfigurationChange::RemoveServer { node_id }
            }
        }
    }
}

impl From<WireConfigurationChange> for ConfigurationChange {
    fn from(wire: WireConfigurationChange) -> Self {
        match wire {
            WireConfigurationChange::AddServer { node_id } => {
                ConfigurationChange::AddServer(node_id)
            }
            WireConfigurationChange::RemoveServer { node_id } => {
                ConfigurationChange::RemoveServer(node_id)
            }
        }
    }
}

/// Serializable entry type for wire protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireEntryType {
    Command { payload: String },
    ConfigChange { change: WireConfigurationChange },
}

impl From<EntryType<String>> for WireEntryType {
    fn from(entry_type: EntryType<String>) -> Self {
        match entry_type {
            EntryType::Command(payload) => WireEntryType::Command { payload },
            EntryType::ConfigChange(change) => WireEntryType::ConfigChange {
                change: WireConfigurationChange::from(change),
            },
        }
    }
}

impl From<WireEntryType> for EntryType<String> {
    fn from(wire: WireEntryType) -> Self {
        match wire {
            WireEntryType::Command { payload } => EntryType::Command(payload),
            WireEntryType::ConfigChange { change } => {
                EntryType::ConfigChange(ConfigurationChange::from(change))
            }
        }
    }
}

/// Serializable log entry for wire protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireLogEntry {
    pub term: u64,
    pub entry_type: WireEntryType,
}

impl From<LogEntry<String>> for WireLogEntry {
    fn from(entry: LogEntry<String>) -> Self {
        Self {
            term: entry.term,
            entry_type: WireEntryType::from(entry.entry_type),
        }
    }
}

impl From<WireLogEntry> for LogEntry<String> {
    fn from(wire: WireLogEntry) -> Self {
        Self {
            term: wire.term,
            entry_type: EntryType::from(wire.entry_type),
        }
    }
}

/// Serializable wire format for RaftMsg
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireRaftMsg {
    RequestVote {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    AppendEntries {
        term: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<WireLogEntry>,
        leader_commit: u64,
    },
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: u64,
    },
    InstallSnapshot {
        term: u64,
        leader_id: u64,
        last_included_index: u64,
        last_included_term: u64,
        offset: u64,
        data: Vec<u8>,
        done: bool,
    },
    InstallSnapshotResponse {
        term: u64,
        success: bool,
    },
    PreVoteRequest {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    PreVoteResponse {
        term: u64,
        vote_granted: bool,
    },
}

impl From<RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>> for WireRaftMsg {
    fn from(msg: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>) -> Self {
        match msg {
            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => WireRaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            },
            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                WireRaftMsg::RequestVoteResponse { term, vote_granted }
            }
            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                let wire_entries: Vec<WireLogEntry> = entries
                    .as_slice()
                    .iter()
                    .map(|entry| WireLogEntry::from(entry.clone()))
                    .collect();
                WireRaftMsg::AppendEntries {
                    term,
                    prev_log_index,
                    prev_log_term,
                    entries: wire_entries,
                    leader_commit,
                }
            }
            RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => WireRaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            },
            RaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            } => WireRaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data: data.as_slice().to_vec(),
                done,
            },
            RaftMsg::InstallSnapshotResponse { term, success } => {
                WireRaftMsg::InstallSnapshotResponse { term, success }
            }
            RaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => WireRaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            },
            RaftMsg::PreVoteResponse { term, vote_granted } => {
                WireRaftMsg::PreVoteResponse { term, vote_granted }
            }
        }
    }
}

impl TryFrom<WireRaftMsg> for RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>> {
    type Error = &'static str;

    fn try_from(wire: WireRaftMsg) -> Result<Self, Self::Error> {
        match wire {
            WireRaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => Ok(RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            }),
            WireRaftMsg::RequestVoteResponse { term, vote_granted } => {
                Ok(RaftMsg::RequestVoteResponse { term, vote_granted })
            }
            WireRaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                let log_entries: Vec<LogEntry<String>> =
                    entries.into_iter().map(LogEntry::from).collect();
                let collection = EmbassyLogEntryCollection::new(&log_entries);
                Ok(RaftMsg::AppendEntries {
                    term,
                    prev_log_index,
                    prev_log_term,
                    entries: collection,
                    leader_commit,
                })
            }
            WireRaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => Ok(RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            }),
            WireRaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            } => {
                let chunk = HeaplessChunkVec::<512>::new(&data);
                Ok(RaftMsg::InstallSnapshot {
                    term,
                    leader_id,
                    last_included_index,
                    last_included_term,
                    offset,
                    data: chunk,
                    done,
                })
            }
            WireRaftMsg::InstallSnapshotResponse { term, success } => {
                Ok(RaftMsg::InstallSnapshotResponse { term, success })
            }
            WireRaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => Ok(RaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            }),
            WireRaftMsg::PreVoteResponse { term, vote_granted } => {
                Ok(RaftMsg::PreVoteResponse { term, vote_granted })
            }
        }
    }
}
