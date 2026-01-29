// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::log_entry::LogEntry;

pub use crate::collections::error::CollectionError;

pub trait LogEntryCollection {
    type Payload: Clone;
    type Iter<'a>: Iterator<Item = &'a LogEntry<Self::Payload>>
    where
        Self: 'a,
        Self::Payload: 'a;

    fn new(entries: &[LogEntry<Self::Payload>]) -> Self;
    fn clear(&mut self);
    fn push(&mut self, entry: LogEntry<Self::Payload>) -> Result<(), CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn as_slice(&self) -> &[LogEntry<Self::Payload>];
    fn iter(&self) -> Self::Iter<'_>;
    fn truncate(&mut self, index: usize);
}
