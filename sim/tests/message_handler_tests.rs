// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Tests for MessageHandler in isolation
//!
//! These tests demonstrate that MessageHandler can be instantiated and used
//! independently of RaftNode through MessageHandlerContext.

use raft_core::{
    collections::configuration::Configuration, components::{
        config_change_manager::ConfigChangeManager,
        election_manager::ElectionManager,
        log_replication_manager::LogReplicationManager,
        message_handler::{ClientError, MessageHandler, MessageHandlerContext},
        snapshot_manager::SnapshotManager,
    }, log_entry::ConfigurationChange, node_state::NodeState, storage::Storage, timer_service::TimerKind
};
use raft_sim::{
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    in_memory_transport::InMemoryTransport, message_broker::MessageBroker,
    no_action_timer::DummyTimer, null_observer::NullObserver,
};
use std::sync::{Arc, Mutex};

fn make_empty_config() -> Configuration<InMemoryNodeCollection> {
    Configuration::new(InMemoryNodeCollection::new())
}

type TestHandler = MessageHandler<
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    InMemoryNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryChunkCollection,
    InMemoryMapCollection,
    DummyTimer,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
>;

fn create_handler() -> TestHandler {
    MessageHandler::new()
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn create_context<'a>(
    node_id: &'a u64,
    role: &'a mut NodeState,
    current_term: &'a mut u64,
    transport: &'a mut InMemoryTransport,
    storage: &'a mut InMemoryStorage,
    state_machine: &'a mut InMemoryStateMachine,
    observer: &'a mut NullObserver<String, InMemoryLogEntryCollection>,
    election: &'a mut ElectionManager<InMemoryNodeCollection, DummyTimer>,
    replication: &'a mut LogReplicationManager<InMemoryMapCollection>,
    config_manager: &'a mut ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection>,
    snapshot_manager: &'a mut SnapshotManager,
) -> MessageHandlerContext<
    'a,
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    InMemoryNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryChunkCollection,
    InMemoryMapCollection,
    DummyTimer,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
> {
    MessageHandlerContext {
        id: node_id,
        role,
        current_term,
        transport,
        storage,
        state_machine,
        observer,
        election,
        replication,
        config_manager,
        snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    }
}

#[test]
fn test_message_handler_start_election() {
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    storage.set_current_term(5);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    handler.start_election(&mut ctx);

    // Should have incremented term and changed role
    assert_eq!(current_term, 6);
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(storage.current_term(), 6);
}

#[test]
fn test_message_handler_start_pre_vote() {
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    storage.set_current_term(5);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    handler.start_pre_vote(&mut ctx);

    // Pre-vote should NOT increment term or change role
    assert_eq!(current_term, 5);
    assert_eq!(role, NodeState::Follower);
    assert_eq!(storage.current_term(), 5);
}

#[test]
fn test_message_handler_reuse_across_operations() {
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );
    handler.start_pre_vote(&mut ctx);

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, 5);

    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );
    handler.start_election(&mut ctx);

    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(current_term, 6);
}

#[test]
fn test_message_handler_handle_election_timer_as_follower() {
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    handler.handle_timer(&mut ctx, TimerKind::Election);

    // With no peers, pre-vote succeeds and follower becomes leader
    assert_eq!(role, NodeState::Leader);
    // Term increments when transitioning to leader
    assert_eq!(current_term, 6);
}

#[test]
fn test_message_handler_handle_election_timer_as_leader() {
    use raft_core::timer_service::TimerKind;

    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    handler.handle_timer(&mut ctx, TimerKind::Election);

    // Leader should ignore election timer
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 5);
}

#[test]
fn test_message_handler_submit_client_command_as_follower_fails() {
    let node_id = 2;
    let mut role = NodeState::Follower;
    let mut current_term = 1;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    let result = handler.submit_client_command(&mut ctx, "test_command".to_string());
    assert_eq!(result, Err(ClientError::NotLeader));
}

#[test]
fn test_message_handler_submit_config_change_as_follower_fails() {
    let node_id = 2;
    let mut role = NodeState::Follower;
    let mut current_term = 1;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    let handler = create_handler();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
    );

    let result = handler.submit_config_change(&mut ctx, ConfigurationChange::AddServer(4));
    assert_eq!(result, Err(ClientError::NotLeader));
}
