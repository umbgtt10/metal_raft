// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::collections::{configuration::Configuration, node_collection::NodeCollection};
use crate::types::{LogIndex, NodeId};

pub trait MapCollection {
    fn new() -> Self;
    fn insert(&mut self, key: NodeId, value: LogIndex);
    fn get(&self, key: NodeId) -> Option<LogIndex>;
    fn remove(&mut self, key: NodeId);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn values(&self) -> impl Iterator<Item = LogIndex> + '_;
    fn clear(&mut self);
    fn compute_median<C: NodeCollection>(
        &self,
        leader_last_index: LogIndex,
        config: &Configuration<C>,
    ) -> Option<LogIndex>;
}
