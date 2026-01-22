use indexmap::IndexMap;
use raft_core::{
    collections::{
        configuration::Configuration, map_collection::MapCollection,
        node_collection::NodeCollection,
    },
    types::{LogIndex, NodeId},
};

pub struct InMemoryMapCollection {
    map: IndexMap<NodeId, LogIndex>,
}

impl MapCollection for InMemoryMapCollection {
    fn new() -> Self {
        InMemoryMapCollection {
            map: IndexMap::new(),
        }
    }

    fn insert(&mut self, key: NodeId, value: LogIndex) {
        self.map.insert(key, value);
    }

    fn get(&self, key: NodeId) -> Option<LogIndex> {
        self.map.get(&key).cloned()
    }

    fn remove(&mut self, key: NodeId) {
        self.map.swap_remove(&key);
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = LogIndex> + '_ {
        self.map.values().cloned()
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn compute_median<C: NodeCollection>(
        &self,
        leader_last_index: LogIndex,
        config: &Configuration<C>,
        catching_up_servers: &Self,
        leader_id: NodeId,
    ) -> Option<LogIndex> {
        let mut indices: Vec<LogIndex> = Vec::new();

        // Add leader's own index
        indices.push(leader_last_index);

        // Add match_index values for voting members only (exclude catching-up servers and leader)
        for member in config.members.iter() {
            // Skip leader (already added)
            if member == leader_id {
                continue;
            }

            // Skip catching-up servers
            if catching_up_servers.get(member).is_some() {
                continue;
            }

            // Get match_index for this member
            // If not in match_index, assume 0 (unresponsive peer)
            if let Some(match_idx) = self.get(member) {
                indices.push(match_idx);
            } else {
                indices.push(0);
            }
        }

        if indices.is_empty() {
            return None;
        }

        // Sort to find median
        indices.sort_unstable();

        // Median is at position that represents majority
        // For N nodes, commit index is at (N-1)/2 position
        let median_idx = (indices.len() - 1) / 2;
        Some(indices[median_idx])
    }
}
