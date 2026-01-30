// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use indexmap::IndexMap;
use raft_core::timer_service::TimerService;
use raft_core::{
    collections::node_collection::NodeCollection,
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    },
    event::Event,
    raft_messages::RaftMsg,
    raft_node::RaftNode,
    raft_node_builder::RaftNodeBuilder,
    types::NodeId,
};
use raft_test_utils::{
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    in_memory_transport::InMemoryTransport, message_broker::MessageBroker,
    mocked_timer_service::MockClock, mocked_timer_service::MockTimerService,
    null_observer::NullObserver,
};
use std::sync::{Arc, Mutex};
type InMemoryTimefullRaftNode = RaftNode<
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    InMemoryNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryChunkCollection,
    InMemoryMapCollection,
    MockTimerService,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
    MockClock,
>;

pub struct TimefullTestCluster {
    nodes: IndexMap<NodeId, InMemoryTimefullRaftNode>,
    broker: Arc<Mutex<MessageBroker<String, InMemoryLogEntryCollection, InMemoryChunkCollection>>>,
    message_log: Vec<(
        NodeId,
        NodeId,
        RaftMsg<String, InMemoryLogEntryCollection, InMemoryChunkCollection>,
    )>,
    clock: MockClock,
    snapshot_threshold: u64,
}

impl TimefullTestCluster {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            broker: Arc::new(Mutex::new(MessageBroker::new())),
            message_log: Vec::new(),
            clock: MockClock::new(),
            snapshot_threshold: 10,
        }
    }

    pub fn with_snapshot_threshold(mut self, threshold: u64) -> Self {
        self.snapshot_threshold = threshold;
        self
    }

    pub fn get_node(&self, id: NodeId) -> &InMemoryTimefullRaftNode {
        &self.nodes[&id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut InMemoryTimefullRaftNode {
        self.nodes.get_mut(&id).unwrap()
    }

    pub fn add_node_with_timeouts(
        &mut self,
        id: NodeId,
        election_timeout_min: u64,
        election_timeout_max: u64,
    ) {
        let transport = InMemoryTransport::new(id, self.broker.clone());

        let timer = MockTimerService::new(
            election_timeout_min,
            election_timeout_max,
            50,
            self.clock.clone(),
            id,
        );

        let peers = InMemoryNodeCollection::new();
        let node = RaftNodeBuilder::new(id, InMemoryStorage::new(), InMemoryStateMachine::new())
            .with_snapshot_threshold(self.snapshot_threshold)
            .with_election(ElectionManager::new(timer))
            .with_replication(LogReplicationManager::new())
            .with_transport(transport, peers, NullObserver::new(), self.clock.clone());

        self.nodes.insert(id, node);
    }

    pub fn add_node(&mut self, id: NodeId) {
        self.add_node_with_timeouts(id, 150, 300);
    }

    pub fn connect_peers(&mut self) {
        let peer_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();
        let node_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();

        for &node_id in &node_ids {
            let mut peers = InMemoryNodeCollection::new();
            for &pid in &peer_ids {
                if pid != node_id {
                    peers.push(pid).ok();
                }
            }

            let old_node = self.nodes.swap_remove(&node_id).unwrap();
            let storage = old_node.storage().clone();
            let state_machine = old_node.state_machine().clone();
            let timer = old_node.timer_service().clone();

            let transport = InMemoryTransport::new(node_id, self.broker.clone());

            let new_node = RaftNodeBuilder::new(node_id, storage, state_machine)
                .with_snapshot_threshold(self.snapshot_threshold)
                .with_election(ElectionManager::new(timer))
                .with_replication(LogReplicationManager::new())
                .with_transport(transport, peers, NullObserver::new(), self.clock.clone());

            self.nodes.insert(node_id, new_node);
        }
    }

    pub fn process_timer_events(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();

        for node_id in node_ids {
            let fired_timers = {
                let node = self.nodes.get(&node_id).unwrap();
                node.timer_service().check_expired()
            };

            for timer_kind in fired_timers.iter() {
                let node = self.nodes.get_mut(&node_id).unwrap();
                node.on_event(Event::TimerFired(timer_kind));
            }
        }
    }

    pub fn deliver_messages(&mut self) {
        loop {
            let mut messages_to_deliver = Vec::new();
            {
                let mut broker = self.broker.lock().unwrap();
                for &node_id in self.nodes.keys() {
                    while let Some((from, msg)) = broker.dequeue(node_id) {
                        messages_to_deliver.push((node_id, from, msg));
                    }
                }
            }

            if messages_to_deliver.is_empty() {
                break;
            }

            for (node_id, from, msg) in messages_to_deliver {
                self.message_log.push((from, node_id, msg.clone()));

                let node = self.nodes.get_mut(&node_id).unwrap();
                node.on_event(Event::Message { from, msg });
            }
        }
    }

    pub fn advance_time(&mut self, millis: u64) {
        self.clock.advance(millis);
        self.process_timer_events();
    }
}

impl Default for TimefullTestCluster {
    fn default() -> Self {
        Self::new()
    }
}
