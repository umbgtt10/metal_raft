// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Cluster configuration management
//!
//! Tracks the set of nodes in the Raft cluster and provides
//! quorum calculation helpers.

use crate::collections::node_collection::NodeCollection;
use crate::types::NodeId;

/// Represents the current cluster configuration
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration<C: NodeCollection> {
    /// All nodes in the cluster, including the local node
    pub members: C,
}

impl<C: NodeCollection> Configuration<C> {
    /// Create a new configuration with the given members
    pub fn new(members: C) -> Self {
        Self { members }
    }

    /// Check if a node is a member of this configuration
    pub fn contains(&self, node_id: NodeId) -> bool {
        self.members.iter().any(|id| id == node_id)
    }

    /// Get the total number of nodes in this configuration
    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Calculate the quorum size (simple majority)
    /// For a cluster of N nodes, quorum = ⌊N/2⌋ + 1
    pub fn quorum_size(&self) -> usize {
        let size = self.size();
        if size == 0 {
            0
        } else {
            (size / 2) + 1
        }
    }

    /// Check if a given count meets the quorum requirement
    pub fn has_quorum(&self, count: usize) -> bool {
        count >= self.quorum_size()
    }

    /// Get an iterator over all members (including self)
    pub fn iter(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.members.iter()
    }

    /// Get an iterator over peer members (excluding the specified node_id)
    pub fn peers(&self, self_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.members.iter().filter(move |&id| id != self_id)
    }
}
