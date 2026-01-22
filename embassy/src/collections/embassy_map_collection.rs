// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use heapless::index_map::FnvIndexMap;
use raft_core::{
    collections::{
        configuration::Configuration, map_collection::MapCollection,
        node_collection::NodeCollection,
    },
    types::{LogIndex, NodeId},
};

#[derive(Debug, Clone)]
pub struct EmbassyMapCollection {
    map: FnvIndexMap<NodeId, LogIndex, 8>,
}

impl MapCollection for EmbassyMapCollection {
    fn new() -> Self {
        Self {
            map: FnvIndexMap::new(),
        }
    }

    fn insert(&mut self, key: NodeId, value: LogIndex) {
        let _ = self.map.insert(key, value);
    }

    fn get(&self, key: NodeId) -> Option<LogIndex> {
        self.map.get(&key).copied()
    }

    fn remove(&mut self, key: NodeId) {
        self.map.remove(&key);
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = LogIndex> + '_ {
        self.map.values().copied()
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn compute_median<C: NodeCollection>(
        &self,
        leader_last_index: LogIndex,
        config: &Configuration<C>,
        catching_up_servers: &Self,
    ) -> Option<LogIndex> {
        let mut indices: heapless::Vec<LogIndex, 10> = heapless::Vec::new();

        // Add leader's own index
        let _ = indices.push(leader_last_index);

        // Add match_index values for voting members only (exclude catching-up servers)
        for member in config.members.iter() {
            // Skip catching-up servers
            if catching_up_servers.get(member).is_some() {
                continue;
            }

            // Get match_index for this member
            if let Some(match_idx) = self.get(member) {
                let _ = indices.push(match_idx);
            } else {
                // Not yet in match_index map
                let _ = indices.push(0);
            }
        }

        if indices.is_empty() {
            return None;
        }

        indices.sort_unstable();

        // Median is at position that represents majority
        let median_idx = (indices.len() - 1) / 2;
        Some(indices[median_idx])
    }
}
