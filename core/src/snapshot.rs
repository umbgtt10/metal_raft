// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::chunk_collection::ChunkCollection,
    types::{LogIndex, Term},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
}

#[derive(Clone, Debug)]
pub struct Snapshot<D: SnapshotData> {
    pub metadata: SnapshotMetadata,
    pub data: D,
}

#[derive(Clone, Debug)]
pub struct SnapshotChunk<D: SnapshotData> {
    pub offset: usize,
    pub data: D::Chunk,
    pub done: bool,
}

pub trait SnapshotData: Clone {
    type Chunk: ChunkCollection + Clone;

    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn chunk_at(&self, offset: usize, max_len: usize) -> Option<Self::Chunk>;
}

pub trait SnapshotBuilder {
    type Output: SnapshotData;
    type ChunkInput: Clone;

    fn new() -> Self;

    fn add_chunk(
        &mut self,
        offset: usize,
        data: Self::ChunkInput,
    ) -> Result<(), SnapshotBuildError>;
    fn is_complete(&self, expected_size: usize) -> bool;
    fn build(self) -> Result<Self::Output, SnapshotBuildError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotBuildError {
    InvalidOffset,
    Incomplete,
    OutOfBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    CorruptData,
    IncompatibleVersion,
    DeserializationFailed,
    NoEntriesToSnapshot,
    EntryNotFound,
    NotLeader,
}
