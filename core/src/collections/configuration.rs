// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::collections::node_collection::NodeCollection;
use crate::types::NodeId;

#[derive(Clone, Debug, PartialEq)]
pub struct Configuration<C: NodeCollection> {
    pub members: C,
}

impl<C: NodeCollection> Configuration<C> {
    pub fn new(members: C) -> Self {
        Self { members }
    }

    pub fn contains(&self, node_id: NodeId) -> bool {
        self.members.iter().any(|id| id == node_id)
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn quorum_size(&self) -> usize {
        let size = self.size();
        if size == 0 {
            0
        } else {
            (size / 2) + 1
        }
    }

    pub fn has_quorum(&self, count: usize) -> bool {
        count >= self.quorum_size()
    }

    pub fn iter(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.members.iter()
    }

    pub fn peers(&self, self_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.members.iter().filter(move |&id| id != self_id)
    }
}
