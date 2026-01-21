// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::collections::config_change_collection::{CollectionError, ConfigChangeCollection};
use raft_core::log_entry::ConfigurationChange;
use raft_core::types::LogIndex;

#[derive(Clone, Debug)]
pub struct InMemoryConfigChangeCollection {
    changes: Vec<(LogIndex, ConfigurationChange)>,
}

impl ConfigChangeCollection for InMemoryConfigChangeCollection {
    type Iter<'a> = InMemoryConfigChangeIter<'a>;

    fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    fn push(
        &mut self,
        index: LogIndex,
        change: ConfigurationChange,
    ) -> Result<(), CollectionError> {
        self.changes.push((index, change));
        Ok(())
    }

    fn len(&self) -> usize {
        self.changes.len()
    }

    fn clear(&mut self) {
        self.changes.clear();
    }

    fn iter(&self) -> Self::Iter<'_> {
        InMemoryConfigChangeIter {
            inner: self.changes.iter(),
        }
    }
}

pub struct InMemoryConfigChangeIter<'a> {
    inner: core::slice::Iter<'a, (LogIndex, ConfigurationChange)>,
}

impl<'a> Iterator for InMemoryConfigChangeIter<'a> {
    type Item = (LogIndex, &'a ConfigurationChange);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(idx, change)| (*idx, change))
    }
}
