// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    snapshot::{Snapshot, SnapshotError, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
    types::LogIndex,
};

pub struct SnapshotManager {
    threshold: LogIndex,
}

impl SnapshotManager {
    pub fn new(threshold: LogIndex) -> Self {
        Self { threshold }
    }

    pub fn threshold(&self) -> LogIndex {
        self.threshold
    }

    pub fn should_create<S>(&self, commit_index: LogIndex, storage: &S) -> bool
    where
        S: Storage,
    {
        if commit_index < self.threshold {
            return false;
        }

        if let Some(snapshot) = storage.load_snapshot() {
            if snapshot.metadata.last_included_index >= self.threshold {
                return false;
            }
        }

        true
    }

    pub fn create<S, SM>(
        &self,
        storage: &mut S,
        state_machine: &mut SM,
        last_applied: LogIndex,
    ) -> Result<LogIndex, SnapshotError>
    where
        S: Storage<SnapshotData = SM::SnapshotData>,
        SM: StateMachine,
    {
        if last_applied == 0 {
            return Err(SnapshotError::NoEntriesToSnapshot);
        }

        let last_included_term = storage
            .get_entry(last_applied)
            .map(|e| e.term)
            .ok_or(SnapshotError::EntryNotFound)?;

        let snapshot_data = state_machine.create_snapshot();

        let snapshot = Snapshot {
            metadata: SnapshotMetadata {
                last_included_index: last_applied,
                last_included_term,
            },
            data: snapshot_data,
        };

        storage.save_snapshot(snapshot);
        self.compact(storage, last_applied);

        Ok(last_applied)
    }

    pub fn compact<S>(&self, storage: &mut S, last_included_index: LogIndex)
    where
        S: Storage,
    {
        storage.discard_entries_before(last_included_index + 1);
    }
}
