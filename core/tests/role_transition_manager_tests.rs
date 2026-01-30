// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::{map_collection::MapCollection, node_collection::NodeCollection},
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
        role_transition_manager::RoleTransitionManager,
    },
    node_state::NodeState,
    observer::Role,
    raft_messages::RaftMsg,
    storage::Storage,
};
use raft_test_utils::{
    frozen_timer::FrozenTimer, in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection, in_memory_storage::InMemoryStorage,
    null_observer::NullObserver,
};

#[test]
fn test_node_state_to_role_follower() {
    let state = NodeState::Follower;
    assert_eq!(
        RoleTransitionManager::node_state_to_role(&state),
        Role::Follower
    );
}

#[test]
fn test_node_state_to_role_candidate() {
    let state = NodeState::Candidate;
    assert_eq!(
        RoleTransitionManager::node_state_to_role(&state),
        Role::Candidate
    );
}

#[test]
fn test_node_state_to_role_leader() {
    let state = NodeState::Leader;
    assert_eq!(
        RoleTransitionManager::node_state_to_role(&state),
        Role::Leader
    );
}

// ============================================================
// start_pre_vote tests
// ============================================================

#[test]
fn test_start_pre_vote_returns_pre_vote_request() {
    let node_id = 1;
    let current_term = 5;
    let storage = InMemoryStorage::new();
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    let msg = RoleTransitionManager::start_pre_vote::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        current_term,
        &storage,
        &mut election,
        &mut observer,
    );

    match msg {
        RaftMsg::PreVoteRequest {
            term, candidate_id, ..
        } => {
            assert_eq!(term, current_term);
            assert_eq!(candidate_id, node_id);
        }
        _ => panic!("Expected PreVoteRequest"),
    }
}

// ============================================================
// start_election tests
// ============================================================

#[test]
fn test_start_election_increments_term() {
    let node_id = 1;
    let mut current_term = 5;
    let mut storage = InMemoryStorage::new();
    let mut role = NodeState::Follower;
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    let _msg = RoleTransitionManager::start_election::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Follower,
    );

    assert_eq!(current_term, 6);
    assert_eq!(storage.current_term(), 6);
}

#[test]
fn test_start_election_changes_role_to_candidate() {
    let node_id = 1;
    let mut current_term = 5;
    let mut storage = InMemoryStorage::new();
    let mut role = NodeState::Follower;
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    let _msg = RoleTransitionManager::start_election::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Follower,
    );

    assert_eq!(role, NodeState::Candidate);
}

#[test]
fn test_start_election_votes_for_self() {
    let node_id = 1;
    let mut current_term = 5;
    let mut storage = InMemoryStorage::new();
    let mut role = NodeState::Follower;
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    let _msg = RoleTransitionManager::start_election::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Follower,
    );

    assert_eq!(storage.voted_for(), Some(node_id));
}

// ============================================================
// become_leader tests
// ============================================================

#[test]
fn test_become_leader_changes_role() {
    let node_id = 1;
    let current_term = 5;
    let mut role = NodeState::Candidate;
    let storage = InMemoryStorage::new();
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut observer = NullObserver::new();

    RoleTransitionManager::become_leader::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        InMemoryMapCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        current_term,
        &mut role,
        &storage,
        InMemoryNodeCollection::new().iter(),
        &mut election,
        &mut replication,
        &mut observer,
        Role::Candidate,
    );

    assert_eq!(role, NodeState::Leader);
}

// ============================================================
// step_down tests
// ============================================================

#[test]
fn test_step_down_from_leader() {
    let node_id = 1;
    let old_term = 5;
    let new_term = 10;
    let mut current_term = 5;
    let mut storage = InMemoryStorage::new();
    let mut role = NodeState::Leader;
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    RoleTransitionManager::step_down::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        old_term,
        new_term,
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Leader,
    );

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, new_term);
    assert_eq!(storage.current_term(), new_term);
    assert_eq!(storage.voted_for(), None);
}

#[test]
fn test_step_down_from_candidate() {
    let node_id = 1;
    let old_term = 5;
    let new_term = 10;
    let mut current_term = 5;
    let mut storage = InMemoryStorage::new();
    let mut role = NodeState::Candidate;
    let mut election = ElectionManager::<InMemoryNodeCollection, _>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    RoleTransitionManager::step_down::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        old_term,
        new_term,
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Candidate,
    );

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, new_term);
    assert_eq!(storage.current_term(), new_term);
    assert_eq!(storage.voted_for(), None);
}

#[test]
fn test_become_leader_initializes_replication() {
    let node_id = 1;
    let current_term = 1;
    let mut role = NodeState::Candidate;
    let storage = InMemoryStorage::new();
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut observer = NullObserver::new();
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();

    RoleTransitionManager::become_leader::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        InMemoryMapCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        current_term,
        &mut role,
        &storage,
        members.iter(),
        &mut election,
        &mut replication,
        &mut observer,
        Role::Candidate,
    );

    assert_eq!(role, NodeState::Leader);
    // Check that replication is initialized for followers
    assert!(replication.next_index().get(2).is_some());
    assert!(replication.match_index().get(2).is_some());
}

#[test]
fn test_become_leader_with_multiple_followers() {
    let node_id = 1;
    let current_term = 1;
    let mut role = NodeState::Candidate;
    let storage = InMemoryStorage::new();
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut observer = NullObserver::new();

    // Create cluster with 5 nodes
    let mut members = InMemoryNodeCollection::new();
    for i in 1..=5 {
        members.push(i).unwrap();
    }

    RoleTransitionManager::become_leader::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        InMemoryMapCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        current_term,
        &mut role,
        &storage,
        members.iter(),
        &mut election,
        &mut replication,
        &mut observer,
        Role::Candidate,
    );

    assert_eq!(role, NodeState::Leader);
    // Check that replication is initialized for all followers
    for follower_id in 2..=5 {
        assert!(replication.next_index().get(follower_id).is_some(),);
        assert!(replication.match_index().get(follower_id).is_some(),);
    }
}

#[test]
fn test_step_down_notifies_observer() {
    let node_id = 1;
    let mut current_term = 1;
    let mut role = NodeState::Leader;
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut observer = NullObserver::new();

    RoleTransitionManager::step_down::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryNodeCollection,
        FrozenTimer,
        NullObserver<String, InMemoryLogEntryCollection>,
        InMemoryStorage,
    >(
        node_id,
        1, // old_term
        3, // new_term
        &mut current_term,
        &mut storage,
        &mut role,
        &mut election,
        &mut observer,
        Role::Leader,
    );

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, 3);
    assert_eq!(storage.current_term(), 3);
}
