// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::{
        configuration::Configuration, log_entry_collection::LogEntryCollection,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    components::{
        config_change_manager::ConfigChangeManager,
        election_manager::ElectionManager,
        leader_lease::LeaderLease,
        log_replication_manager::LogReplicationManager,
        message_handler::{ClientError, MessageHandler},
        message_handler_context::MessageHandlerContext,
        snapshot_manager::SnapshotManager,
    },
    log_entry::{ConfigurationChange, EntryType, LogEntry},
    node_state::NodeState,
    raft_messages::RaftMsg,
    storage::Storage,
    timer_service::TimerKind,
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn create_context<'a>(
    node_id: &'a u64,
    role: &'a mut NodeState,
    current_term: &'a mut u64,
    current_leader: &'a mut Option<u64>,
    transport: &'a mut InMemoryTransport,
    storage: &'a mut InMemoryStorage,
    state_machine: &'a mut InMemoryStateMachine,
    observer: &'a mut NullObserver<String, InMemoryLogEntryCollection>,
    election: &'a mut ElectionManager<InMemoryNodeCollection, FrozenTimer>,
    replication: &'a mut LogReplicationManager<InMemoryMapCollection>,
    config_manager: &'a mut ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection>,
    snapshot_manager: &'a mut SnapshotManager,
    leader_lease: &'a mut LeaderLease<FrozenClock>,
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
    FrozenTimer,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
    FrozenClock,
> {
    MessageHandlerContext {
        id: node_id,
        role,
        current_term,
        current_leader,
        transport,
        storage,
        state_machine,
        observer,
        election,
        replication,
        config_manager,
        snapshot_manager,
        leader_lease,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    }
}

#[test]
fn test_message_handler_start_election() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    storage.set_current_term(5);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.start_election(&mut ctx);

    // Assert
    assert_eq!(current_term, 6);
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(storage.current_term(), 6);
}

#[test]
fn test_message_handler_start_pre_vote() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    storage.set_current_term(5);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.start_pre_vote(&mut ctx);

    // Assert
    assert_eq!(current_term, 5);
    assert_eq!(role, NodeState::Follower);
    assert_eq!(storage.current_term(), 5);
}

#[test]
fn test_message_handler_reuse_across_operations() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.start_pre_vote(&mut ctx);

    // Assert
    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, 5);

    // Act
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    handler.start_election(&mut ctx);

    // Assert
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(current_term, 6);
}

#[test]
fn test_message_handler_handle_election_timer_as_follower() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.handle_timer(&mut ctx, TimerKind::Election);

    // Assert
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 6);
}

#[test]
fn test_message_handler_handle_election_timer_as_leader() {
    // Arrange
    use raft_core::timer_service::TimerKind;

    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.handle_timer(&mut ctx, TimerKind::Election);

    // Assert
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 5);
}

#[test]
fn test_message_handler_handle_election_timer_as_candidate() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Candidate;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    storage.set_voted_for(Some(1));
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.handle_timer(&mut ctx, TimerKind::Election);

    // Assert
    assert_eq!(*ctx.role, NodeState::Candidate);
    assert_eq!(*ctx.current_term, 5);
    assert_eq!(storage.current_term(), 5);
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handle_heartbeat_timer_as_leader() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.handle_timer(&mut ctx, TimerKind::Heartbeat);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_submit_client_command_as_follower_fails() {
    // Arrange
    let node_id = 2;
    let mut role = NodeState::Follower;
    let mut current_term = 1;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    let result = handler.submit_client_command(&mut ctx, "test_command".to_string());

    // Assert
    assert_eq!(result, Err(ClientError::NotLeader { leader_hint: None }));
}

#[test]
fn test_message_handler_submit_config_change_as_follower_fails() {
    // Arrange
    let node_id = 2;
    let mut role = NodeState::Follower;
    let mut current_term = 1;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    let result = handler.submit_config_change(&mut ctx, ConfigurationChange::AddServer(4));

    // Assert
    assert_eq!(result, Err(ClientError::NotLeader { leader_hint: None }));
}

#[test]
fn test_message_handler_handles_higher_term_message() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 1;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::AppendEntries {
        term: 2,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(current_term, 2);
    assert_eq!(role, NodeState::Follower);
}

#[test]
fn test_message_handler_handles_pre_vote_request() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::PreVoteRequest {
        term: 2,
        candidate_id: 2,
        last_log_index: 0,
        last_log_term: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_vote_request() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::RequestVote {
        term: 2,
        candidate_id: 2,
        last_log_index: 0,
        last_log_term: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(storage.voted_for(), Some(2));
}

#[test]
fn test_message_handler_handles_append_entries_success() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::AppendEntries {
        term: 2,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_broadcast_to_peers() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    members.push(3).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.send_heartbeats(&mut ctx);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
    assert!(messages.peak(3).is_some());
}

#[test]
fn test_message_handler_send_append_entries_to_followers() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    handler.send_append_entries_to_followers(&mut ctx);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_vote_response_updates_election_state() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Candidate;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.set_voted_for(Some(1));
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    members.push(3).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, _>(
        node_id,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::RequestVoteResponse {
        term: 2,
        vote_granted: true,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
}

#[test]
fn test_message_handler_handles_install_snapshot_message() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager =
        ConfigChangeManager::new(Configuration::new(InMemoryNodeCollection::new()));
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::InstallSnapshot {
        term: 2,
        leader_id: 2,
        last_included_index: 5,
        last_included_term: 1,
        offset: 0,
        data: InMemoryChunkCollection::new(),
        done: true,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_install_snapshot_response() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::InstallSnapshotResponse {
        term: 2,
        success: true,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(role, NodeState::Leader);
}

#[test]
fn test_message_handler_submit_client_command_as_leader_succeeds() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    // Act
    let result = handler.submit_client_command(&mut ctx, "test_command".to_string());

    // Assert
    assert!(result.is_ok());
}

#[test]
fn test_message_handler_submit_config_change_as_leader_succeeds() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let config_change = ConfigurationChange::AddServer(2);

    // Act
    let result = handler.submit_config_change(&mut ctx, config_change);

    // Assert
    assert!(result.is_ok());
}

#[test]
fn test_message_handler_handles_append_entries_response() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );

    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert!(replication.match_index().get(2).is_some());
}

#[test]
fn test_message_handler_ignores_stale_append_entries_response() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 3;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let initial_match_index = replication.match_index().get(2);
    let initial_commit_index = replication.commit_index();
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 10,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(replication.match_index().get(2), initial_match_index);
    assert_eq!(replication.commit_index(), initial_commit_index);
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 3);
}

#[test]
fn test_message_handler_grants_lease_on_commit_advance() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let payload = "test command".to_string();
    let log_entry = LogEntry {
        term: 2,
        entry_type: EntryType::Command(payload.clone()),
    };
    storage.append_entries(&[log_entry]);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    assert!(!ctx.leader_lease.is_valid());
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(ctx.replication.commit_index(), 1);
    assert!(ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_revokes_lease_on_step_down() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    ctx.leader_lease.grant();
    assert!(ctx.leader_lease.is_valid());
    let msg = RaftMsg::AppendEntries {
        term: 3,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert_eq!(*ctx.role, NodeState::Follower);
    assert_eq!(*ctx.current_term, 3);
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_does_not_grant_lease_for_non_leader() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    assert!(!ctx.leader_lease.is_valid());
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_does_not_grant_lease_for_stale_term() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 3;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    assert!(!ctx.leader_lease.is_valid());
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };

    // Act
    handler.handle_message(&mut ctx, 2, msg);

    // Assert
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_lease_remains_valid_across_multiple_commits() {
    // Arrange
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 2;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);
    let entries = vec![
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ];
    for entry in entries {
        storage.append_entries(&[entry]);
    }
    let handler = MessageHandler::new();
    let mut ctx = create_context(
        &node_id,
        &mut role,
        &mut current_term,
        &mut current_leader,
        &mut transport,
        &mut storage,
        &mut state_machine,
        &mut observer,
        &mut election,
        &mut replication,
        &mut config_manager,
        &mut snapshot_manager,
        &mut leader_lease,
    );
    assert!(!ctx.leader_lease.is_valid());

    // Act
    let msg1 = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };
    handler.handle_message(&mut ctx, 2, msg1);

    // Assert
    assert_eq!(ctx.replication.commit_index(), 1);
    assert!(ctx.leader_lease.is_valid());

    // Act
    let msg2 = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 2,
    };
    handler.handle_message(&mut ctx, 2, msg2);

    // Assert
    assert_eq!(ctx.replication.commit_index(), 2);
    assert!(ctx.leader_lease.is_valid());
}
