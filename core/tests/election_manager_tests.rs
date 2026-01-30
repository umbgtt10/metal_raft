// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::{configuration::Configuration, node_collection::NodeCollection},
    components::election_manager::ElectionManager,
    log_entry::{EntryType, LogEntry},
    node_state::NodeState,
    raft_messages::RaftMsg,
    snapshot::{Snapshot, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
};

use raft_test_utils::{
    frozen_timer::FrozenTimer, in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
};

#[test]
fn test_liveness_start_election_increments_term() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    // Act
    let _msg: RaftMsg<String, InMemoryLogEntryCollection, InMemoryChunkCollection> =
        election.start_election(1, &mut current_term, &mut storage, &mut role);

    // Assert
    assert_eq!(current_term, 2);
    assert_eq!(role, NodeState::Candidate);
    assert_eq!(storage.current_term(), 2);
}

#[test]
fn test_safety_start_election_votes_for_self() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    // Act
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Assert
    assert_eq!(storage.voted_for(), Some(1));
}

#[test]
fn test_liveness_start_election_generates_correct_message() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Follower;

    // Act
    let msg = election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        42,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Assert
    match msg {
        RaftMsg::RequestVote {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        } => {
            assert_eq!(term, 6);
            assert_eq!(candidate_id, 42);
            assert_eq!(last_log_index, 0);
            assert_eq!(last_log_term, 0);
        }
        _ => panic!("Expected RequestVote message"),
    }
}

#[test]
fn test_safety_grant_vote_to_first_candidate() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        5,
        0,
        0,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { term, vote_granted } => {
            assert_eq!(term, 1);
            assert!(vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
    assert_eq!(storage.voted_for(), Some(5));
}

#[test]
fn test_safety_reject_when_already_voted() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.set_voted_for(Some(3));
    let mut current_term = 1;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        5,
        0,
        0,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_vote_to_same_candidate_twice() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.set_voted_for(Some(5));
    let mut current_term = 1;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        5,
        0,
        0,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_reject_less_up_to_date_candidate() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd3".to_string()),
        },
    ]);
    storage.set_current_term(2);
    let mut current_term = 2;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        2,
        5,
        2,
        2,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_to_equally_up_to_date_candidate() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);
    storage.set_current_term(2);
    let mut current_term = 2;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        2,
        5,
        2,
        2,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                vote_granted,
                "Should grant vote to equally up-to-date candidate"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_update_term_from_higher_request() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.set_voted_for(Some(3));
    let mut current_term = 1;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        5,
        7,
        0,
        0,
        &mut current_term,
        &mut storage,
    );

    // Assert
    assert_eq!(current_term, 5);
    assert_eq!(storage.current_term(), 5);
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(vote_granted);
            assert_eq!(storage.voted_for(), Some(7));
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_reject_stale_vote_request() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        3,
        7,
        0,
        0,
        &mut current_term,
        &mut storage,
    );

    // Assert
    match response {
        RaftMsg::RequestVoteResponse { term, vote_granted } => {
            assert_eq!(term, 5);
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}
#[test]
fn test_handle_pre_vote_request_log_not_up_to_date() {
    // Arrange
    let timer_service = FrozenTimer;
    let mut manager = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(timer_service);
    let mut storage = InMemoryStorage::new();
    storage.append_entries(&[LogEntry {
        entry_type: EntryType::Command("cmd".to_string()),
        term: 2,
    }]);

    // Act
    let msg = manager.handle_pre_vote_request::<
        String,
        InMemoryLogEntryCollection,
        InMemoryChunkCollection,
        InMemoryStorage,
    >(1, 1, 1, 1, &storage);

    // Assert
    match msg {
        RaftMsg::PreVoteResponse { term, vote_granted } => {
            assert_eq!(term, 1);
            assert!(!vote_granted);
        }
        _ => panic!("Expected PreVoteResponse"),
    }
}

#[test]
fn test_liveness_become_leader_on_majority() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());

    // Act
    let became_leader = election.handle_vote_response(2, 3, true, &current_term, &role, &config);

    // Assert
    assert!(became_leader);
}

#[test]
fn test_liveness_stay_candidate_without_majority() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let mut peers = InMemoryNodeCollection::new();
    for i in 2..=5 {
        peers.push(i).unwrap();
    }
    let config = Configuration::new(peers.clone());

    // Act
    let became_leader = election.handle_vote_response(2, 3, true, &current_term, &role, &config);

    // Assert
    assert!(!became_leader);
    assert_eq!(role, NodeState::Candidate);
}

#[test]
fn test_liveness_handle_vote_rejection() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());

    // Act
    let became_leader = election.handle_vote_response(2, 3, false, &current_term, &role, &config);

    // Assert
    assert!(!became_leader);
    assert_eq!(role, NodeState::Candidate);
}

#[test]
fn test_safety_ignore_stale_vote_response() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());

    // Act
    let became_leader = election.handle_vote_response(2, 3, true, &current_term, &role, &config);

    // Assert
    assert!(!became_leader);
}

#[test]
fn test_liveness_majority_calculation_even_cluster() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    peers.push(4).unwrap();
    let config = Configuration::new(peers.clone());

    // Act
    election.handle_vote_response(2, 2, true, &current_term, &role, &config);
    let became_leader = election.handle_vote_response(3, 2, true, &current_term, &role, &config);

    // Assert
    assert!(became_leader);
}

#[test]
fn test_liveness_single_node_cluster() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Candidate;
    election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );
    let peers = InMemoryNodeCollection::new();
    let config = Configuration::new(peers.clone());

    // Act
    let became_leader =
        election.handle_vote_response(1, current_term, true, &current_term, &role, &config);
    if became_leader {
        role = NodeState::Leader;
    }

    // Assert
    assert!(became_leader);
    assert_eq!(role, NodeState::Leader);
}

#[test]
fn test_liveness_start_election_with_compacted_log() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);
    storage.discard_entries_before(11);
    storage.discard_entries_before(16);

    // Act
    let msg = election.start_election::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Assert
    assert_eq!(storage.first_log_index(), 16);
    assert_eq!(storage.last_log_index(), 15);
    assert_eq!(storage.last_log_term(), 2);
    match msg {
        RaftMsg::RequestVote {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        } => {
            assert_eq!(term, 4);
            assert_eq!(candidate_id, 1);
            assert_eq!(last_log_index, 15);
            assert_eq!(last_log_term, 2);
        }
        _ => panic!("Expected RequestVote message"),
    }
}

#[test]
fn test_safety_grant_vote_with_compacted_log() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);

    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);
    storage.discard_entries_before(21);
    let actual_last_index = storage.last_log_index();
    let actual_last_term = storage.last_log_term();

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        3,
        5,
        18,
        2,
        &mut current_term,
        &mut storage,
    );

    // Assert
    assert_eq!(actual_last_index, 20, "last_log_index when log empty");
    assert_eq!(actual_last_term, 2, "last_log_term from snapshot");
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_reject_candidate_with_older_log_than_snapshot() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);
    storage.discard_entries_before(16);

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        3,
        5,
        10,
        2,
        &mut current_term,
        &mut storage,
    );

    // Assert
    assert_eq!(storage.last_log_index(), 15);
    assert_eq!(storage.last_log_term(), 2);
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_vote_with_higher_term_than_snapshot() {
    // Arrange
    let mut election: ElectionManager<InMemoryNodeCollection, FrozenTimer> =
        ElectionManager::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);
    storage.discard_entries_before(16);

    // Act
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        3,
        5,
        10,
        3,
        &mut current_term,
        &mut storage,
    );

    // Assert
    assert_eq!(storage.last_log_index(), 15);
    assert_eq!(storage.last_log_term(), 2);
    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_liveness_pre_vote_rejection_with_stale_term() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let current_term = 5;

    // Act
    let response = election.handle_pre_vote_request::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        3, 0, 0, current_term, &storage,
    );

    // Assert
    match response {
        RaftMsg::PreVoteResponse { vote_granted, .. } => {
            assert!(!vote_granted);
        }
        _ => panic!("Expected PreVoteResponse"),
    }
}

#[test]
fn test_safety_pre_vote_response_from_minority_does_not_start_election() {
    // Arrange
    let mut election = ElectionManager::<InMemoryNodeCollection, FrozenTimer>::new(FrozenTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let current_term = 2;
    election.start_pre_vote::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(
        1,
        current_term,
        &storage,
    );
    let mut peers = InMemoryNodeCollection::new();
    for i in 2..=5 {
        peers.push(i).unwrap();
    }
    let config = Configuration::new(peers);

    // Act
    let should_start_election = election.handle_pre_vote_response(2, true, &config);

    // Assert
    assert!(!should_start_election);
    assert_eq!(current_term, 2);
}
