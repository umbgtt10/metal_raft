// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::collections::embassy_log_collection::EmbassyLogEntryCollection;
use crate::collections::heapless_chunk_collection::HeaplessChunkVec;
use crate::embassy_state_machine::EmbassySnapshotData;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec as AllocVec;
use core::fmt::Write;
use cortex_m_semihosting::hio;
use heapless::Vec;
use raft_core::{
    collections::{chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection},
    log_entry::{EntryType, LogEntry},
    snapshot::{
        Snapshot, SnapshotBuildError, SnapshotBuilder, SnapshotChunk, SnapshotData, SnapshotError,
        SnapshotMetadata,
    },
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

#[derive(Clone)]
pub struct SemihostingStorage {
    node_id: NodeId,
    current_term: Term,
    voted_for: Option<NodeId>,
    log: AllocVec<LogEntry<String>>,
    snapshot: Option<Snapshot<EmbassySnapshotData>>,
    first_log_index: LogIndex,
}

impl SemihostingStorage {
    pub fn new(node_id: u64) -> Self {
        let mut storage = Self {
            node_id,
            current_term: 0,
            voted_for: None,
            log: AllocVec::new(),
            snapshot: None,
            first_log_index: 1,
        };

        storage.load_from_disk();
        storage
    }

    fn metadata_path(&self) -> AllocVec<u8> {
        format!("persistency/raft_node_{}_metadata.bin\0", self.node_id).into_bytes()
    }

    fn log_path(&self) -> AllocVec<u8> {
        format!("persistency/raft_node_{}_log.bin\0", self.node_id).into_bytes()
    }

    fn snapshot_path(&self) -> AllocVec<u8> {
        format!("persistency/raft_node_{}_snapshot.bin\0", self.node_id).into_bytes()
    }

    fn snapshot_meta_path(&self) -> AllocVec<u8> {
        format!("persistency/raft_node_{}_snapshot_meta.bin\0", self.node_id).into_bytes()
    }

    fn load_from_disk(&mut self) {
        if let Ok(data) = self.read_file(&self.metadata_path()) {
            if data.len() >= 17 {
                self.current_term = u64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);

                let has_vote = data[8] != 0;
                if has_vote {
                    self.voted_for = Some(u64::from_le_bytes([
                        data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                        data[16],
                    ]));
                }
            }
        }

        if let Ok(data) = self.read_file(&self.log_path()) {
            self.log = self.deserialize_log(&data);
        }

        if let Ok(meta_data) = self.read_file(&self.snapshot_meta_path()) {
            if meta_data.len() >= 16 {
                let last_included_index = u64::from_le_bytes([
                    meta_data[0],
                    meta_data[1],
                    meta_data[2],
                    meta_data[3],
                    meta_data[4],
                    meta_data[5],
                    meta_data[6],
                    meta_data[7],
                ]);
                let last_included_term = u64::from_le_bytes([
                    meta_data[8],
                    meta_data[9],
                    meta_data[10],
                    meta_data[11],
                    meta_data[12],
                    meta_data[13],
                    meta_data[14],
                    meta_data[15],
                ]);

                if let Ok(snapshot_data) = self.read_file(&self.snapshot_path()) {
                    let mut data = Vec::new();
                    for byte in snapshot_data.iter().take(512) {
                        if data.push(*byte).is_err() {
                            break;
                        }
                    }

                    self.snapshot = Some(Snapshot {
                        metadata: SnapshotMetadata {
                            last_included_index,
                            last_included_term,
                        },
                        data: EmbassySnapshotData { data },
                    });

                    self.first_log_index = last_included_index + 1;
                }
            }
        }
    }

    fn persist_metadata(&self) {
        let mut data = AllocVec::new();

        data.extend_from_slice(&self.current_term.to_le_bytes());

        if let Some(vote) = self.voted_for {
            data.push(1);
            data.extend_from_slice(&vote.to_le_bytes());
        } else {
            data.push(0);
            data.extend_from_slice(&[0u8; 8]);
        }

        let _ = self.write_file(&self.metadata_path(), &data);
    }

    fn persist_log(&self) {
        let data = self.serialize_log(&self.log);
        let _ = self.write_file(&self.log_path(), &data);
    }

    fn persist_snapshot(&self) {
        if let Some(snapshot) = &self.snapshot {
            let mut meta_data = AllocVec::new();
            meta_data.extend_from_slice(&snapshot.metadata.last_included_index.to_le_bytes());
            meta_data.extend_from_slice(&snapshot.metadata.last_included_term.to_le_bytes());
            let _ = self.write_file(&self.snapshot_meta_path(), &meta_data);

            let _ = self.write_file(&self.snapshot_path(), snapshot.data.data.as_slice());
        }
    }

    fn serialize_log(&self, log: &[LogEntry<String>]) -> AllocVec<u8> {
        let mut data = AllocVec::new();

        data.extend_from_slice(&(log.len() as u64).to_le_bytes());

        for entry in log {
            data.extend_from_slice(&entry.term.to_le_bytes());

            match &entry.entry_type {
                EntryType::Command(payload) => {
                    data.push(0);
                    let payload_bytes = payload.as_bytes();
                    data.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
                    data.extend_from_slice(payload_bytes);
                }
                EntryType::ConfigChange(_) => {
                    data.push(1);
                    data.extend_from_slice(&0u32.to_le_bytes());
                }
            }
        }

        data
    }

    fn deserialize_log(&self, data: &[u8]) -> AllocVec<LogEntry<String>> {
        let mut log = AllocVec::new();

        if data.len() < 8 {
            return log;
        }

        let count = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;

        let mut offset = 8;

        for _ in 0..count {
            if offset + 8 > data.len() {
                break;
            }

            let term = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;

            if offset >= data.len() {
                break;
            }

            let entry_type = data[offset];
            offset += 1;

            if offset + 4 > data.len() {
                break;
            }

            let payload_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if entry_type == 0 {
                // Command
                if offset + payload_len > data.len() {
                    break;
                }

                if let Ok(payload) = String::from_utf8(data[offset..offset + payload_len].to_vec())
                {
                    log.push(LogEntry {
                        term,
                        entry_type: EntryType::Command(payload),
                    });
                }
                offset += payload_len;
            } else {
                // ConfigChange (skip for now)
                offset += payload_len;
            }
        }

        log
    }

    /// Write data to file via semihosting using hio module
    fn write_file(&self, path: &[u8], data: &[u8]) -> Result<(), ()> {
        // Convert path to string for hio
        let path_str = core::str::from_utf8(path).map_err(|_| ())?;

        // Use hstdout for debugging
        if let Ok(mut stdout) = hio::hstdout() {
            let _ = writeln!(
                stdout,
                "Writing {} bytes to {}",
                data.len(),
                path_str.trim_end_matches('\0')
            );
        }

        unsafe {
            let mode = 0x0000_0004 | 0x0000_0002; // MODE_WRITE | MODE_BINARY
            let fd = cortex_m_semihosting::syscall!(OPEN, path.as_ptr(), mode, path.len() - 1);

            if fd == usize::MAX {
                return Err(());
            }

            // SYS_WRITE (0x05): Write to file
            let result = cortex_m_semihosting::syscall!(WRITE, fd, data.as_ptr(), data.len());

            // SYS_CLOSE (0x02): Close file
            cortex_m_semihosting::syscall!(CLOSE, fd);

            if result == 0 {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    fn read_file(&self, path: &[u8]) -> Result<AllocVec<u8>, ()> {
        let path_str = core::str::from_utf8(path).map_err(|_| ())?;

        if let Ok(mut stdout) = hio::hstdout() {
            let _ = writeln!(stdout, "Reading from {}", path_str.trim_end_matches('\0'));
        }

        unsafe {
            // SYS_OPEN (0x01): Open file for reading
            let mode = 0x0000_0000; // MODE_READ
            let fd = cortex_m_semihosting::syscall!(OPEN, path.as_ptr(), mode, path.len() - 1);

            if fd == usize::MAX {
                return Err(());
            }

            // SYS_FLEN (0x0C): Get file length
            let len = cortex_m_semihosting::syscall!(FLEN, fd);

            if len == usize::MAX {
                cortex_m_semihosting::syscall!(CLOSE, fd);
                return Err(());
            }

            // Read data
            #[allow(clippy::slow_vector_initialization)]
            let mut data = AllocVec::with_capacity(len);
            data.resize(len, 0);

            let bytes_read = cortex_m_semihosting::syscall!(READ, fd, data.as_mut_ptr(), len);

            // SYS_CLOSE (0x02): Close file
            cortex_m_semihosting::syscall!(CLOSE, fd);

            if bytes_read == 0 {
                Ok(data)
            } else {
                Err(())
            }
        }
    }
}

pub struct EmbassySnapshotBuilder {
    data: Vec<u8, 512>,
}

impl SnapshotBuilder for EmbassySnapshotBuilder {
    type Output = EmbassySnapshotData;
    type ChunkInput = HeaplessChunkVec<512>;

    fn new() -> Self {
        EmbassySnapshotBuilder { data: Vec::new() }
    }

    fn add_chunk(
        &mut self,
        _offset: usize,
        chunk: Self::ChunkInput,
    ) -> Result<(), SnapshotBuildError> {
        // Append chunk data to our buffer
        for byte in chunk.as_slice() {
            self.data
                .push(*byte)
                .map_err(|_| SnapshotBuildError::OutOfBounds)?;
        }
        Ok(())
    }

    fn is_complete(&self, expected_size: usize) -> bool {
        self.data.len() >= expected_size
    }

    fn build(self) -> Result<Self::Output, raft_core::snapshot::SnapshotBuildError> {
        Ok(EmbassySnapshotData { data: self.data })
    }
}

impl Storage for SemihostingStorage {
    type Payload = String;
    type LogEntryCollection = EmbassyLogEntryCollection;
    type SnapshotData = EmbassySnapshotData;
    type SnapshotChunk = HeaplessChunkVec<512>;
    type SnapshotBuilder = EmbassySnapshotBuilder;

    fn current_term(&self) -> Term {
        self.current_term
    }

    fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
        self.persist_metadata(); // Persist after change
    }

    fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    fn set_voted_for(&mut self, vote: Option<NodeId>) {
        self.voted_for = vote;
        self.persist_metadata(); // Persist after change
    }

    fn last_log_index(&self) -> LogIndex {
        if self.log.is_empty() {
            // If no entries, return the last compacted index
            if let Some(snapshot) = &self.snapshot {
                snapshot.metadata.last_included_index
            } else {
                0
            }
        } else {
            self.first_log_index + self.log.len() as LogIndex - 1
        }
    }

    fn last_log_term(&self) -> Term {
        if let Some(last_entry) = self.log.last() {
            last_entry.term
        } else if let Some(snapshot) = &self.snapshot {
            snapshot.metadata.last_included_term
        } else {
            0
        }
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>> {
        if index < self.first_log_index {
            None // Entry was compacted
        } else {
            let offset = (index - self.first_log_index) as usize;
            self.log.get(offset).cloned()
        }
    }

    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection {
        if start >= end || start < self.first_log_index {
            return EmbassyLogEntryCollection::new(&[]);
        }

        let start_offset = (start - self.first_log_index) as usize;
        let end_offset = (end - self.first_log_index) as usize;

        if start_offset >= self.log.len() {
            return EmbassyLogEntryCollection::new(&[]);
        }

        // Clamp to available entries to prevent out of bounds
        let actual_end_offset = end_offset.min(self.log.len());

        let slice = &self.log[start_offset..actual_end_offset];
        EmbassyLogEntryCollection::new(slice)
    }

    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]) {
        for entry in entries {
            self.log.push(entry.clone());
        }
        self.persist_log(); // Persist after change
    }

    fn truncate_after(&mut self, index: LogIndex) {
        if index < self.first_log_index {
            // Truncate all entries
            self.log.clear();
        } else {
            let offset = (index - self.first_log_index + 1) as usize;
            self.log.truncate(offset);
        }
        self.persist_log(); // Persist after change
    }

    // === Snapshot Methods ===

    fn save_snapshot(&mut self, snapshot: Snapshot<Self::SnapshotData>) {
        self.snapshot = Some(snapshot);
        self.persist_snapshot(); // Persist after change
    }

    fn load_snapshot(&self) -> Option<Snapshot<Self::SnapshotData>> {
        self.snapshot.clone()
    }

    fn snapshot_metadata(&self) -> Option<SnapshotMetadata> {
        self.snapshot.as_ref().map(|s| s.metadata.clone())
    }

    fn get_snapshot_chunk(
        &self,
        offset: usize,
        max_size: usize,
    ) -> Option<SnapshotChunk<Self::SnapshotData>> {
        let snapshot = self.snapshot.as_ref()?;
        let chunk_data = snapshot.data.chunk_at(offset, max_size)?;

        let done = offset + chunk_data.len() >= snapshot.data.len();

        Some(SnapshotChunk {
            offset,
            data: chunk_data,
            done,
        })
    }

    fn begin_snapshot_transfer(&mut self) -> Self::SnapshotBuilder {
        EmbassySnapshotBuilder::new()
    }

    fn apply_snapshot_chunk(
        &mut self,
        offset: u64,
        chunk: Self::SnapshotChunk,
        done: bool,
        last_included_index: LogIndex,
        last_included_term: Term,
    ) -> Result<(), SnapshotError> {
        if offset == 0 && done {
            let mut data = Vec::new();
            for byte in chunk.as_slice() {
                data.push(*byte).map_err(|_| SnapshotError::CorruptData)?;
            }
            let snapshot_data = EmbassySnapshotData { data };
            let snapshot = Snapshot {
                metadata: SnapshotMetadata {
                    last_included_index,
                    last_included_term,
                },
                data: snapshot_data,
            };
            self.snapshot = Some(snapshot);
            self.persist_snapshot();
            Ok(())
        } else {
            Err(SnapshotError::CorruptData)
        }
    }

    fn finalize_snapshot(
        &mut self,
        builder: Self::SnapshotBuilder,
        metadata: SnapshotMetadata,
    ) -> Result<(), SnapshotError> {
        let data = builder.build().map_err(|_| SnapshotError::CorruptData)?;
        self.snapshot = Some(Snapshot { metadata, data });
        self.persist_snapshot();
        Ok(())
    }

    fn discard_entries_before(&mut self, index: LogIndex) {
        if index <= self.first_log_index {
            return;
        }

        let num_to_discard = (index - self.first_log_index) as usize;

        if num_to_discard >= self.log.len() {
            self.log.clear();
        } else {
            let mut new_log = AllocVec::new();
            for i in num_to_discard..self.log.len() {
                new_log.push(self.log[i].clone());
            }
            self.log = new_log;
        }

        self.first_log_index = index;
        self.persist_log();
    }

    fn first_log_index(&self) -> LogIndex {
        self.first_log_index
    }
}

impl Default for SemihostingStorage {
    fn default() -> Self {
        Self::new(0)
    }
}
