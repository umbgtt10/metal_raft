// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        chunk_collection::ChunkCollection, config_change_collection::ConfigChangeCollection,
        configuration::Configuration, log_entry_collection::LogEntryCollection,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    log_entry::EntryType,
    node_state::NodeState,
    observer::Observer,
    raft_messages::RaftMsg,
    snapshot::SnapshotBuilder,
    state_machine::StateMachine,
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

pub struct LogReplicationManager<M>
where
    M: MapCollection,
{
    next_index: M,
    match_index: M,
    commit_index: LogIndex,
    last_applied: LogIndex,
    catching_up_servers: M,
}

impl<M> LogReplicationManager<M>
where
    M: MapCollection,
{
    pub fn new() -> Self {
        Self {
            next_index: M::new(),
            match_index: M::new(),
            commit_index: 0,
            last_applied: 0,
            catching_up_servers: M::new(),
        }
    }

    pub fn commit_index(&self) -> LogIndex {
        self.commit_index
    }

    pub fn last_applied(&self) -> LogIndex {
        self.last_applied
    }

    pub fn set_last_applied(&mut self, index: LogIndex) {
        self.last_applied = index;
    }

    pub fn next_index(&self) -> &M {
        &self.next_index
    }

    pub fn next_index_mut(&mut self) -> &mut M {
        &mut self.next_index
    }

    pub fn match_index(&self) -> &M {
        &self.match_index
    }

    pub fn match_index_mut(&mut self) -> &mut M {
        &mut self.match_index
    }

    pub fn initialize_leader_state<P, L, S, I>(&mut self, peers: I, storage: &S)
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        I: Iterator<Item = NodeId>,
    {
        let next_log_index = storage.last_log_index() + 1;
        for peer in peers {
            self.next_index.insert(peer, next_log_index);
            self.match_index.insert(peer, 0);
        }
    }

    pub fn get_append_entries_for_peer<P, L, C, S>(
        &self,
        peer: NodeId,
        leader_id: NodeId,
        storage: &S,
    ) -> RaftMsg<P, L, C>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = C> + Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        C: ChunkCollection + Clone,
    {
        let next_idx = self.next_index.get(peer).unwrap_or(1);
        let first_idx = storage.first_log_index();
        let current_term = storage.current_term();

        if next_idx < first_idx {
            if let Some(chunk) = storage.get_snapshot_chunk(0, usize::MAX) {
                if let Some(metadata) = storage.snapshot_metadata() {
                    return RaftMsg::InstallSnapshot {
                        term: current_term,
                        leader_id,
                        last_included_index: metadata.last_included_index,
                        last_included_term: metadata.last_included_term,
                        offset: chunk.offset as u64,
                        data: chunk.data,
                        done: chunk.done,
                    };
                }
            }
        }

        self.get_append_entries_for_follower(peer, current_term, storage)
    }

    pub fn get_append_entries_for_follower<P, L, C, S>(
        &self,
        peer: NodeId,
        current_term: Term,
        storage: &S,
    ) -> RaftMsg<P, L, C>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        C: ChunkCollection + Clone,
    {
        let next_idx = self.next_index.get(peer).unwrap_or(1);
        let prev_log_index = next_idx.saturating_sub(1);
        let prev_log_term = if prev_log_index == 0 {
            0
        } else if let Some(entry) = storage.get_entry(prev_log_index) {
            entry.term
        } else if let Some(snapshot) = storage.load_snapshot() {
            if prev_log_index == snapshot.metadata.last_included_index {
                snapshot.metadata.last_included_term
            } else {
                0
            }
        } else {
            0
        };

        let entries = storage.get_entries(next_idx, storage.last_log_index() + 1);

        RaftMsg::AppendEntries {
            term: current_term,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_append_entries<P, L, C, CC, S, SM, O>(
        &mut self,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
        current_term: &mut Term,
        storage: &mut S,
        state_machine: &mut SM,
        role: &mut NodeState,
        observer: &mut O,
        node_id: NodeId,
    ) -> (RaftMsg<P, L, C>, CC)
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        SM: StateMachine<Payload = P>,
        L: LogEntryCollection<Payload = P> + Clone,
        C: ChunkCollection + Clone,
        CC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = C>,
    {
        if term > *current_term {
            *current_term = term;
            storage.set_current_term(term);
            *role = NodeState::Follower;
            storage.set_voted_for(None);
        } else if term == *current_term && *role == NodeState::Candidate {
            *role = NodeState::Follower;
        }

        let (success, config_changes) = if term < *current_term {
            (false, CC::new())
        } else {
            match self.try_append_entries(
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
                storage,
                state_machine,
                observer,
                node_id,
            ) {
                Some(config_changes) => (true, config_changes),
                None => (false, CC::new()),
            }
        };

        let response = RaftMsg::AppendEntriesResponse {
            term: *current_term,
            success,
            match_index: storage.last_log_index(),
        };

        (response, config_changes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_append_entries_response<P, L, S, SM, C, CC, O, CHK>(
        &mut self,
        from: NodeId,
        success: bool,
        match_index: LogIndex,
        storage: &S,
        state_machine: &mut SM,
        config: &Configuration<C>,
        leader_id: NodeId,
        observer: &mut O,
        node_id: NodeId,
    ) -> CC
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
        C: NodeCollection,
        CC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CHK>,
        CHK: ChunkCollection + Clone,
    {
        if success {
            let current_match = self.match_index.get(from).unwrap_or(0);
            if match_index > current_match {
                self.match_index.insert(from, match_index);
                self.next_index.insert(from, match_index + 1);

                self.check_and_promote_caught_up_server(from);

                return self.advance_commit_index(
                    storage,
                    state_machine,
                    config,
                    leader_id,
                    observer,
                    node_id,
                );
            }
        } else {
            let next = self.next_index.get(from).unwrap_or(1);
            if next > 1 {
                self.next_index.insert(from, next - 1);
            }
        }

        CC::new()
    }

    #[allow(clippy::too_many_arguments)]
    fn try_append_entries<P, L, S, SM, CC, O, CHK>(
        &mut self,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
        storage: &mut S,
        state_machine: &mut SM,
        observer: &mut O,
        node_id: NodeId,
    ) -> Option<CC>
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
        L: LogEntryCollection<Payload = P>,
        CC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CHK>,
        CHK: ChunkCollection + Clone,
    {
        if !self.check_log_consistency(prev_log_index, prev_log_term, storage) {
            return None;
        }

        if !entries.is_empty() {
            let last_index = storage.last_log_index();
            if last_index > prev_log_index {
                storage.truncate_after(prev_log_index);
            }
            storage.append_entries(entries.as_slice());
        }

        if leader_commit > self.commit_index {
            self.commit_index = leader_commit.min(storage.last_log_index());
            let config_changes =
                self.apply_committed_entries(storage, state_machine, observer, node_id);
            return Some(config_changes);
        }

        Some(CC::new())
    }

    fn check_log_consistency<P, L, S>(
        &self,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        storage: &S,
    ) -> bool
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        if prev_log_index == 0 {
            return true;
        }

        if let Some(entry) = storage.get_entry(prev_log_index) {
            return entry.term == prev_log_term;
        }

        if let Some(snapshot_metadata) = storage.snapshot_metadata() {
            if prev_log_index == snapshot_metadata.last_included_index {
                return prev_log_term == snapshot_metadata.last_included_term;
            }
        }

        false
    }

    fn apply_committed_entries<P, L, S, SM, CC, O, CHK>(
        &mut self,
        storage: &S,
        state_machine: &mut SM,
        observer: &mut O,
        node_id: NodeId,
    ) -> CC
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
        CC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CHK>,
        CHK: ChunkCollection + Clone,
    {
        let mut config_changes = CC::new();

        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = storage.get_entry(self.last_applied) {
                match &entry.entry_type {
                    EntryType::Command(ref payload) => {
                        state_machine.apply(payload);
                        observer.state_machine_applied(node_id, self.last_applied, payload);
                    }
                    EntryType::ConfigChange(ref change) => {
                        let _ = config_changes.push(self.last_applied, change.clone());
                    }
                }
            }
        }

        config_changes
    }

    pub fn advance_commit_index<P, L, S, SM, C, CC, O, CHK>(
        &mut self,
        storage: &S,
        state_machine: &mut SM,
        config: &Configuration<C>,
        leader_id: NodeId,
        observer: &mut O,
        node_id: NodeId,
    ) -> CC
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
        C: NodeCollection,
        CC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CHK>,
        CHK: ChunkCollection + Clone,
    {
        let leader_index = storage.last_log_index();

        if let Some(new_commit) = self.match_index.compute_median(
            leader_index,
            config,
            &self.catching_up_servers,
            leader_id,
        ) {
            if new_commit > self.commit_index {
                if let Some(entry) = storage.get_entry(new_commit) {
                    if entry.term == storage.current_term() {
                        self.commit_index = new_commit;
                        return self.apply_committed_entries(
                            storage,
                            state_machine,
                            observer,
                            node_id,
                        );
                    }
                }
            }
        }

        CC::new()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_install_snapshot<P, L, C, S, SM>(
        &mut self,
        term: Term,
        _leader_id: NodeId,
        last_included_index: LogIndex,
        last_included_term: Term,
        offset: u64,
        data: C,
        done: bool,
        current_term: &mut Term,
        storage: &mut S,
        state_machine: &mut SM,
        role: &mut NodeState,
    ) -> RaftMsg<P, L, C>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = C>,
        L: LogEntryCollection<Payload = P> + Clone,
        C: ChunkCollection + Clone,
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S::SnapshotBuilder: SnapshotBuilder<Output = S::SnapshotData, ChunkInput = C>,
    {
        if term < *current_term {
            return RaftMsg::InstallSnapshotResponse {
                term: *current_term,
                success: false,
            };
        }

        if term > *current_term {
            *current_term = term;
            storage.set_current_term(term);
            storage.set_voted_for(None);
            *role = NodeState::Follower;
        } else if term == *current_term && *role == NodeState::Candidate {
            *role = NodeState::Follower;
        }

        if let Some(current_snapshot_metadata) = storage.snapshot_metadata() {
            if last_included_index <= current_snapshot_metadata.last_included_index {
                return RaftMsg::InstallSnapshotResponse {
                    term: *current_term,
                    success: false,
                };
            }
        }

        match storage.apply_snapshot_chunk(
            offset,
            data,
            done,
            last_included_index,
            last_included_term,
        ) {
            Ok(_) => {
                if done {
                    if let Some(snapshot) = storage.load_snapshot() {
                        let _ = state_machine.restore_from_snapshot(&snapshot.data);

                        storage.discard_entries_before(last_included_index + 1);

                        self.commit_index = self.commit_index.max(last_included_index);
                        self.last_applied = self.last_applied.max(last_included_index);
                    }
                }

                RaftMsg::InstallSnapshotResponse {
                    term: *current_term,
                    success: true,
                }
            }
            Err(_) => RaftMsg::InstallSnapshotResponse {
                term: *current_term,
                success: false,
            },
        }
    }

    pub fn handle_install_snapshot_response(
        &mut self,
        peer: NodeId,
        success: bool,
        last_included_index: LogIndex,
    ) {
        if success {
            self.next_index.insert(peer, last_included_index + 1);
            self.match_index.insert(peer, last_included_index);

            self.check_and_promote_caught_up_server(peer);
        }
    }

    pub fn mark_server_catching_up(&mut self, node_id: NodeId) {
        self.catching_up_servers.insert(node_id, 1);
    }

    fn check_and_promote_caught_up_server(&mut self, node_id: NodeId) {
        if self.catching_up_servers.get(node_id).is_some() {
            let match_idx = self.match_index.get(node_id).unwrap_or(0);

            if match_idx >= self.commit_index {
                self.catching_up_servers.remove(node_id);
            }
        }
    }

    pub fn is_catching_up(&self, node_id: NodeId) -> bool {
        self.catching_up_servers.get(node_id).is_some()
    }
}

impl<M: MapCollection> Default for LogReplicationManager<M> {
    fn default() -> Self {
        Self::new()
    }
}
