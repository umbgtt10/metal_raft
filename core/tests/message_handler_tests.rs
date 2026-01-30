// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Tests for MessageHandler in isolation
//!
//! These tests demonstrate that MessageHandler can be instantiated and used
//! independently of RaftNode through MessageHandlerContext.

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
    FrozenTimer,
    NullObserver<String, InMemoryLogEntryCollection>,
    InMemoryConfigChangeCollection,
    FrozenClock,
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    storage.set_current_term(5);

    let handler = create_handler();
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
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    storage.set_current_term(5);

    let handler = create_handler();
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
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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
    handler.start_pre_vote(&mut ctx);

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, 5);

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

    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(current_term, 6);
}

#[test]
fn test_message_handler_handle_election_timer_as_follower() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    handler.handle_timer(&mut ctx, TimerKind::Election);

    // Leader should ignore election timer
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 5);
}

#[test]
fn test_message_handler_handle_election_timer_as_candidate() {
    let node_id = 1;
    let mut role = NodeState::Candidate;
    let mut current_term = 5;
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker.clone());
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    storage.set_voted_for(Some(1)); // Voted for self in current term
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Config with peers so we don't win election immediately
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    // Candidate timeout -> triggers Pre-Vote (term stays same)
    handler.handle_timer(&mut ctx, TimerKind::Election);

    assert_eq!(*ctx.role, NodeState::Candidate);
    assert_eq!(*ctx.current_term, 5);
    assert_eq!(storage.current_term(), 5);

    // Should broadcast PreVoteRequest
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handle_heartbeat_timer_as_leader() {
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

    // Config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Init leader state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Heartbeat timer fires
    handler.handle_timer(&mut ctx, TimerKind::Heartbeat);

    // Should broadcast Heartbeat (AppendEntries)
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_submit_client_command_as_follower_fails() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    let result = handler.submit_client_command(&mut ctx, "test_command".to_string());
    assert_eq!(result, Err(ClientError::NotLeader { leader_hint: None }));
}

#[test]
fn test_message_handler_submit_config_change_as_follower_fails() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    let result = handler.submit_config_change(&mut ctx, ConfigurationChange::AddServer(4));
    assert_eq!(result, Err(ClientError::NotLeader { leader_hint: None }));
}

#[test]
fn test_message_handler_handles_higher_term_message() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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
        term: 2, // Higher term
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    handler.handle_message(&mut ctx, 2, msg);

    assert_eq!(current_term, 2); // Term updated
    assert_eq!(role, NodeState::Follower); // Became follower
}

#[test]
fn test_message_handler_handles_pre_vote_request() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Should send response message
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_vote_request() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Should send response and grant vote (first request)
    assert_eq!(storage.voted_for(), Some(2));
}

#[test]
fn test_message_handler_handles_append_entries_success() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Should send response
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_broadcast_to_peers() {
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

    // Create config with multiple peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    members.push(3).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    // Send heartbeats (broadcasts to all peers)
    handler.send_heartbeats(&mut ctx);

    // Should have messages for both peers
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
    assert!(messages.peak(3).is_some());
}

#[test]
fn test_message_handler_send_append_entries_to_followers() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    handler.send_append_entries_to_followers(&mut ctx);

    // Should send AppendEntries to follower
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_vote_response_updates_election_state() {
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

    // Create config with 3 nodes
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    members.push(3).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Start election first to initialize vote tracking
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, _>(
        node_id,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    let handler = create_handler();
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

    // Handle vote response
    let msg = RaftMsg::RequestVoteResponse {
        term: 2,
        vote_granted: true,
    };

    handler.handle_message(&mut ctx, 2, msg);

    // Vote response is handled (exact behavior depends on internal logic)
    // The important part is it doesn't crash
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
}

#[test]
fn test_message_handler_handles_install_snapshot_message() {
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
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Should send response
    let messages = broker.lock().unwrap();
    assert!(messages.peak(2).is_some());
}

#[test]
fn test_message_handler_handles_install_snapshot_response() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Response is processed (doesn't crash)
    assert_eq!(role, NodeState::Leader);
}

#[test]
fn test_message_handler_submit_client_command_as_leader_succeeds() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Submit command as leader should succeed
    let result = handler.submit_client_command(&mut ctx, "test_command".to_string());

    assert!(result.is_ok());
}

#[test]
fn test_message_handler_submit_config_change_as_leader_succeeds() {
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

    // Create config with existing nodes
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Submit config change as leader should succeed
    let config_change = ConfigurationChange::AddServer(2);
    let result = handler.submit_config_change(&mut ctx, config_change);

    assert!(result.is_ok());
}

#[test]
fn test_message_handler_handles_append_entries_response() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    handler.handle_message(&mut ctx, 2, msg);

    // Should update replication state
    assert!(replication.match_index().get(2).is_some());
}

#[test]
fn test_message_handler_ignores_stale_append_entries_response() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);
    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let initial_match_index = replication.match_index().get(2);
    let initial_commit_index = replication.commit_index();

    let handler = create_handler();
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

    // Stale term response should be ignored
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 10,
    };

    handler.handle_message(&mut ctx, 2, msg);

    assert_eq!(replication.match_index().get(2), initial_match_index);
    assert_eq!(replication.commit_index(), initial_commit_index);
    assert_eq!(role, NodeState::Leader);
    assert_eq!(current_term, 3);
}

#[test]
fn test_message_handler_grants_lease_on_commit_advance() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);

    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    // Add a log entry to replicate
    let payload = "test command".to_string();
    let log_entry = LogEntry {
        term: 2,
        entry_type: EntryType::Command(payload.clone()),
    };
    storage.append_entries(&[log_entry]);

    let handler = create_handler();
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

    // Initially lease should be invalid
    assert!(!ctx.leader_lease.is_valid());

    // Send append entries response that advances commit index to 1
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1, // Follower has replicated the entry at index 1
    };

    handler.handle_message(&mut ctx, 2, msg);

    // Commit index should advance to 1 (quorum of 2 nodes)
    assert_eq!(ctx.replication.commit_index(), 1);

    // Lease should now be valid
    assert!(ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_revokes_lease_on_step_down() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);

    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Grant lease manually for this test
    ctx.leader_lease.grant();
    assert!(ctx.leader_lease.is_valid());

    // Simulate receiving a higher term message that causes step down
    let msg = RaftMsg::AppendEntries {
        term: 3,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    handler.handle_message(&mut ctx, 2, msg);

    // Should have stepped down
    assert_eq!(*ctx.role, NodeState::Follower);
    assert_eq!(*ctx.current_term, 3);

    // Lease should be revoked
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_does_not_grant_lease_for_non_leader() {
    let node_id = 1;
    let mut role = NodeState::Follower; // Not a leader
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);

    // Initialize replication state (even though we're not leader)
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Initially lease should be invalid
    assert!(!ctx.leader_lease.is_valid());

    // Send append entries response - should be ignored since we're not leader
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };

    handler.handle_message(&mut ctx, 2, msg);

    // Lease should still be invalid (not granted)
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_does_not_grant_lease_for_stale_term() {
    let node_id = 1;
    let mut role = NodeState::Leader;
    let mut current_term = 3; // Higher term than the response
    let mut current_leader = None;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);

    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    let handler = create_handler();
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

    // Initially lease should be invalid
    assert!(!ctx.leader_lease.is_valid());

    // Send append entries response with stale term (2 < 3)
    let msg = RaftMsg::AppendEntriesResponse {
        term: 2, // Stale term
        success: true,
        match_index: 1,
    };

    handler.handle_message(&mut ctx, 2, msg);

    // Lease should still be invalid (not granted due to stale term)
    assert!(!ctx.leader_lease.is_valid());
}

#[test]
fn test_message_handler_lease_remains_valid_across_multiple_commits() {
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

    // Create config with peers
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut config_manager = ConfigChangeManager::new(config);
    let mut snapshot_manager = SnapshotManager::new(10);
    let mut leader_lease = LeaderLease::new(5000, FrozenClock);

    // Initialize replication state
    replication.initialize_leader_state(config_manager.config().members.iter(), &storage);

    // Add multiple log entries
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

    let handler = create_handler();
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

    // Initially lease should be invalid
    assert!(!ctx.leader_lease.is_valid());

    // First response - advance commit to 1
    let msg1 = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 1,
    };
    handler.handle_message(&mut ctx, 2, msg1);
    assert_eq!(ctx.replication.commit_index(), 1);
    assert!(ctx.leader_lease.is_valid());

    // Second response - advance commit to 2
    let msg2 = RaftMsg::AppendEntriesResponse {
        term: 2,
        success: true,
        match_index: 2,
    };
    handler.handle_message(&mut ctx, 2, msg2);
    assert_eq!(ctx.replication.commit_index(), 2);

    // Lease should still be valid (not revoked by subsequent commits)
    assert!(ctx.leader_lease.is_valid());
}
