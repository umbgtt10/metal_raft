// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use indexmap::IndexMap;
use raft_core::{
    collections::node_collection::NodeCollection,
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    },
    event::Event,
    raft_node::RaftNode,
    raft_node_builder::RaftNodeBuilder,
    types::NodeId,
};
use raft_test_utils::{
    frozen_timer::{FrozenClock, FrozenTimer},
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine,
    in_memory_storage::InMemoryStorage,
    in_memory_transport::InMemoryTransport,
    message_broker::MessageBroker,
    null_observer::NullObserver,
};
use std::sync::{Arc, Mutex};

pub type TestNode = RaftNode<
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    InMemoryNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryChunkCollection,
    InMemoryMapCollection,
    FrozenTimer,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
    FrozenClock,
>;

pub struct TimelessTestCluster {
    nodes: IndexMap<NodeId, TestNode>,
    message_broker:
        Arc<Mutex<MessageBroker<String, InMemoryLogEntryCollection, InMemoryChunkCollection>>>,
    message_log: Vec<(
        NodeId,
        NodeId,
        raft_core::raft_messages::RaftMsg<
            String,
            InMemoryLogEntryCollection,
            InMemoryChunkCollection,
        >,
    )>,
    partition_groups: Option<(Vec<NodeId>, Vec<NodeId>)>,
    snapshot_threshold: u64,
}

impl TimelessTestCluster {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            message_broker: Arc::new(Mutex::new(MessageBroker::new())),
            message_log: Vec::new(),
            partition_groups: None,
            snapshot_threshold: 10,
        }
    }

    pub fn with_nodes(n: usize) -> Self {
        let mut cluster = Self::new();
        for i in 1..=n {
            cluster.add_node(i as NodeId);
        }
        cluster.connect_peers();
        cluster
    }

    pub fn with_snapshot_threshold(mut self, threshold: u64) -> Self {
        self.snapshot_threshold = threshold;
        self
    }

    pub fn add_node(&mut self, id: NodeId) {
        self.add_node_with_storage(id, InMemoryStorage::new());
    }

    pub fn add_node_with_storage(&mut self, id: NodeId, storage: InMemoryStorage) {
        let transport = InMemoryTransport::new(id, self.message_broker.clone());

        let node = RaftNodeBuilder::new(id, storage, InMemoryStateMachine::new())
            .with_snapshot_threshold(self.snapshot_threshold)
            .with_election(ElectionManager::new(FrozenTimer))
            .with_replication(LogReplicationManager::new())
            .with_transport(
                transport,
                InMemoryNodeCollection::new(),
                NullObserver::new(),
                FrozenClock,
            );

        self.nodes.insert(id, node);
    }

    pub fn connect_peers(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.keys().copied().collect();

        for &node_id in &node_ids {
            let mut expected_peers = InMemoryNodeCollection::new();
            for &peer_id in &node_ids {
                if peer_id != node_id {
                    expected_peers.push(peer_id).unwrap();
                }
            }

            if let Some(current_peers) = self.nodes[&node_id].peers() {
                if current_peers.len() == expected_peers.len() {
                    let mut peers_match = true;
                    for peer_id in expected_peers.iter() {
                        let mut found = false;
                        for current_peer_id in current_peers.iter() {
                            if current_peer_id == peer_id {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            peers_match = false;
                            break;
                        }
                    }
                    if peers_match {
                        continue;
                    }
                }
            }

            let old_node = self.nodes.swap_remove(&node_id).unwrap();
            let storage = old_node.storage().clone();
            let state_machine = old_node.state_machine().clone();
            let transport = InMemoryTransport::new(node_id, self.message_broker.clone());

            let new_node = RaftNodeBuilder::new(node_id, storage, state_machine)
                .with_snapshot_threshold(self.snapshot_threshold)
                .with_election(ElectionManager::new(FrozenTimer))
                .with_replication(LogReplicationManager::new())
                .with_transport(transport, expected_peers, NullObserver::new(), FrozenClock);

            self.nodes.insert(node_id, new_node);
        }
    }

    pub fn deliver_messages(&mut self) {
        loop {
            let mut messages_to_deliver = Vec::new();
            {
                let mut broker = self.message_broker.lock().unwrap();
                for &node_id in self.nodes.keys() {
                    while let Some((from, msg)) = broker.dequeue(node_id) {
                        let should_deliver = if let Some((group1, group2)) = &self.partition_groups
                        {
                            (group1.contains(&from) && group1.contains(&node_id))
                                || (group2.contains(&from) && group2.contains(&node_id))
                        } else {
                            true
                        };

                        if should_deliver {
                            messages_to_deliver.push((node_id, from, msg));
                        }
                    }
                }
            }

            if messages_to_deliver.is_empty() {
                break;
            }

            for (to, from, msg) in messages_to_deliver {
                self.message_log.push((from, to, msg.clone()));
                if let Some(node) = self.nodes.get_mut(&to) {
                    node.on_event(Event::Message { from, msg });
                }
            }
        }
    }

    pub fn get_node(&self, id: NodeId) -> &TestNode {
        self.nodes.get(&id).expect("Node not found")
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut TestNode {
        self.nodes.get_mut(&id).expect("Node not found")
    }

    pub fn get_node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    pub fn get_messages(
        &self,
        recipient: NodeId,
    ) -> Vec<
        raft_core::raft_messages::RaftMsg<
            String,
            InMemoryLogEntryCollection,
            InMemoryChunkCollection,
        >,
    > {
        self.message_log
            .iter()
            .filter(|(_, to, _)| *to == recipient)
            .map(|(_, _, msg)| msg.clone())
            .collect()
    }

    pub fn get_all_messages(
        &self,
    ) -> &[(
        NodeId,
        NodeId,
        raft_core::raft_messages::RaftMsg<
            String,
            InMemoryLogEntryCollection,
            InMemoryChunkCollection,
        >,
    )] {
        &self.message_log
    }

    pub fn message_count(&self) -> usize {
        self.message_log.len()
    }

    pub fn get_messages_from(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> Vec<
        raft_core::raft_messages::RaftMsg<
            String,
            InMemoryLogEntryCollection,
            InMemoryChunkCollection,
        >,
    > {
        self.message_log
            .iter()
            .filter(|(f, t, _)| *f == from && *t == to)
            .map(|(_, _, msg)| msg.clone())
            .collect()
    }

    pub fn clear_message_log(&mut self) {
        self.message_log.clear();
    }

    pub fn deliver_message_from_to(&mut self, from: NodeId, to: NodeId) {
        let mut broker = self.message_broker.lock().unwrap();
        if let Some((sender, msg)) = broker.dequeue_from(to, from) {
            drop(broker); // Release lock before processing
            self.message_log.push((sender, to, msg.clone()));
            if let Some(node) = self.nodes.get_mut(&to) {
                node.on_event(Event::Message { from: sender, msg });
            }
        }
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.swap_remove(&id);
    }

    pub fn reconnect_node(&mut self, _id: NodeId) {
        self.connect_peers();
    }

    pub fn partition(&mut self, group1: &[NodeId], group2: &[NodeId]) {
        self.partition_groups = Some((group1.to_vec(), group2.to_vec()));
    }

    pub fn heal_partition(&mut self) {
        self.partition_groups = None;
    }

    pub fn partition_node(&mut self, node_id: NodeId) {
        let other_nodes: Vec<NodeId> = self
            .nodes
            .keys()
            .copied()
            .filter(|&id| id != node_id)
            .collect();
        self.partition_groups = Some((vec![node_id], other_nodes));
    }

    pub fn heal_partition_node(&mut self, _node_id: NodeId) {
        self.partition_groups = None;
    }

    pub fn deliver_messages_partition(&mut self, partition: &[NodeId]) {
        loop {
            let mut messages_to_deliver = Vec::new();
            {
                let mut broker = self.message_broker.lock().unwrap();
                for &node_id in partition {
                    while let Some((from, msg)) = broker.dequeue(node_id) {
                        if partition.contains(&from) {
                            messages_to_deliver.push((node_id, from, msg));
                        }
                    }
                }
            }

            if messages_to_deliver.is_empty() {
                break;
            }

            for (to, from, msg) in messages_to_deliver {
                self.message_log.push((from, to, msg.clone()));
                if let Some(node) = self.nodes.get_mut(&to) {
                    node.on_event(Event::Message { from, msg });
                }
            }
        }
    }
}

impl Default for TimelessTestCluster {
    fn default() -> Self {
        Self::new()
    }
}
