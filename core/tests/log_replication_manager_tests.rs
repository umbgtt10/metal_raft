// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::{
        config_change_collection::ConfigChangeCollection, configuration::Configuration,
        log_entry_collection::LogEntryCollection, map_collection::MapCollection,
        node_collection::NodeCollection,
    },
    components::log_replication_manager::LogReplicationManager,
    log_entry::{ConfigurationChange, EntryType, LogEntry},
    node_state::NodeState,
    raft_messages::RaftMsg,
    snapshot::{Snapshot, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
};
use raft_test_utils::{
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    null_observer::NullObserver,
};

#[test]
fn test_liveness_accept_entries_from_leader() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        0,
        0,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse { term, success, .. } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
    assert_eq!(storage.last_log_index(), 2);
    assert_eq!(replication.commit_index(), 2);
}

#[test]
fn test_liveness_append_entries_with_config_change_emits_change() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::ConfigChange(ConfigurationChange::AddServer(2)),
        },
    ]);

    // Act
    let (_, config_changes) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        0,
        0,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(config_changes.len(), 1);
    let (index, change) = config_changes.iter().next().expect("Missing config change");
    assert_eq!(index, 2);
    assert_eq!(change, &ConfigurationChange::AddServer(2));
}

#[test]
fn test_safety_reject_entries_from_stale_term() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("cmd1".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        0,
        0,
        entries,
        1,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse { term, success, .. } => {
            assert_eq!(term, 5);
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }

    assert_eq!(storage.last_log_index(), 0);
}

#[test]
fn test_safety_reject_inconsistent_prev_log() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 2,
        entry_type: EntryType::Command("cmd".to_string()),
    }]);
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        5,
        2,
        entries,
        1,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse { success, .. } => {
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
}

#[test]
fn test_safety_reject_mismatched_prev_log_term() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    storage.append_entries(&[LogEntry {
        term: 1,
        entry_type: EntryType::Command("old".to_string()),
    }]);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("new".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        1,
        2,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse { success, .. } => {
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
}

#[test]
fn test_safety_delete_conflicting_entries() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd3".to_string()),
        },
    ]);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 2,
        entry_type: EntryType::Command("new_cmd2".to_string()),
    }]);

    // Act
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        1,
        1,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(storage.last_log_index(), 2);
    let log_entries = storage.get_entries(1, 3);
    assert_eq!(log_entries.len(), 2);
    assert_eq!(log_entries.as_slice()[0].term, 1);
    assert_eq!(log_entries.as_slice()[1].term, 2);
}

#[test]
fn test_liveness_heartbeat_updates_commit_index() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[]);

    // Act
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        2,
        2,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 2);
}

#[test]
fn test_safety_step_down_on_higher_term_append_entries() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Leader;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[]);

    // Act
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        5,
        0,
        0,
        entries,
        0,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(current_term, 5);
    assert_eq!(role, NodeState::Follower);
}

#[test]
fn test_liveness_update_next_index_on_success() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
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
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd4".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd5".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
            2,
            true,
            3,
            &storage,
            &mut state_machine,
            &config,
            1,
            &mut observer,
            1,
        );

    // Assert
    assert_eq!(replication.next_index().get(2), Some(4));
    assert_eq!(replication.match_index().get(2), Some(3));
}

#[test]
fn test_liveness_decrement_next_index_on_failure() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[LogEntry {
        term: 2,
        entry_type: EntryType::Command("cmd".to_string()),
    }]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
            2,
            false,
            0,
            &storage,
            &mut state_machine,
            &config,
            1,
            &mut observer,
            1,
        );

    // Assert
    assert_eq!(replication.next_index().get(2), Some(1));
}

#[test]
fn test_liveness_commit_index_advancement_on_majority() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
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
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd4".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd5".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    peers.push(4).unwrap();
    peers.push(5).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 3, &storage, &mut state_machine, &config, 1, &mut observer, 1);
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(3, true, 3, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 3);

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(4, true, 5, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 3);

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(5, true, 4, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 4);
}

#[test]
fn test_safety_only_commit_current_term_entries() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("old1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("old2".to_string()),
        },
        LogEntry {
            term: 3,
            entry_type: EntryType::Command("new1".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 2, &storage, &mut state_machine, &config, 1, &mut observer, 1);
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(3, true, 2, &storage, &mut state_machine, &config, 1, &mut observer, 1);
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 3, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 3);
}

#[test]
fn test_safety_ignore_responses_from_old_term() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    storage.append_entries(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("cmd".to_string()),
    }]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let initial_next = replication.next_index().get(2);
    let initial_match = replication.match_index().get(2);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
            2,
            true,
            5,
            &storage,
            &mut state_machine,
            &config,
            1,
            &mut observer,
            1,
        );

    // Assert
    assert!(replication.next_index().get(2) >= initial_next);
    assert!(replication.match_index().get(2) >= initial_match);
}

#[test]
fn test_safety_median_no_double_count_leader() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    for i in 1..=10 {
        storage.append_entries(&[LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
    }
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    replication.match_index_mut().insert(2, 3);
    replication.match_index_mut().insert(3, 4);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication.advance_commit_index::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 4);
    assert_eq!(state_machine.get("does_not_exist"), None);
}

#[test]
fn test_liveness_initialize_leader_state() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    for i in 1..=10 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
    }
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    peers.push(4).unwrap();

    // Act
    replication.initialize_leader_state(peers.iter(), &storage);

    // Assert
    assert_eq!(replication.next_index().get(2), Some(11));
    assert_eq!(replication.next_index().get(3), Some(11));
    assert_eq!(replication.next_index().get(4), Some(11));
    assert_eq!(replication.match_index().get(2), Some(0));
    assert_eq!(replication.match_index().get(3), Some(0));
    assert_eq!(replication.match_index().get(4), Some(0));
}

#[test]
fn test_safety_append_entries_with_exact_match() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
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
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);

    // Act
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        0,
        0,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(storage.last_log_index(), 2);
}

#[test]
fn test_safety_commit_index_never_decreases() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries = InMemoryLogEntryCollection::new(&[]);

    // Act
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        2,
        2,
        entries,
        2,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 2);

    // Act
    let entries2 = InMemoryLogEntryCollection::new(&[]);
    replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        2,
        2,
        entries2,
        1,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 2);
}

#[test]
fn test_liveness_three_node_cluster_majority() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 2, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 2);
}

#[test]
fn test_liveness_get_append_entries_with_compacted_snapshot_point() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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

    // Act
    storage.discard_entries_before(11);

    // Assert
    assert_eq!(storage.first_log_index(), 11);
    assert_eq!(storage.last_log_index(), 15);
    assert!(storage.get_entry(10).is_none());

    // Act
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 10, &storage, &mut state_machine, &config, 1, &mut observer, 1);
    let msg = replication.get_append_entries_for_follower::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(2, 3, &storage);

    // Assert
    match msg {
        RaftMsg::AppendEntries {
            term,
            prev_log_index,
            prev_log_term,
            entries: _,
            leader_commit: _,
        } => {
            assert_eq!(term, 3);
            assert_eq!(prev_log_index, 10);
            assert_eq!(prev_log_term, 2);
        }
        _ => panic!("Expected AppendEntries message"),
    }
}

#[test]
fn test_safety_get_append_entries_before_snapshot_point() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    for _ in 0..7 {
        replication
            .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, false, 0, &storage, &mut state_machine, &config, 1, &mut observer, 1);
    }

    // Act
    let msg = replication.get_append_entries_for_follower::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(2, 3, &storage);

    // Assert
    match msg {
        RaftMsg::AppendEntries {
            term,
            prev_log_index,
            prev_log_term,
            entries: _,
            leader_commit: _,
        } => {
            assert_eq!(term, 3);
            assert!(prev_log_index < 11);
            assert_eq!(prev_log_term, 0);
        }
        _ => panic!("Expected AppendEntries message"),
    }
}

#[test]
fn test_safety_follower_rejects_append_with_compacted_prev_log() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            entry_type: raft_core::log_entry::EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    for i in 1..=15 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    assert_eq!(storage.first_log_index(), 16);
    assert!(storage.get_entry(10).is_none());
    let leader_entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("new_cmd".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        10,
        2,
        leader_entries,
        20,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index: _,
            ..
        } => {
            assert_eq!(term, 3);
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
}

#[test]
fn test_liveness_follower_accepts_append_at_snapshot_point() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    assert_eq!(storage.first_log_index(), 11);
    assert_eq!(storage.last_log_index(), 15);
    let leader_entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("cmd16".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        10,
        2,
        leader_entries,
        16,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index,
            ..
        } => {
            assert_eq!(term, 3);
            assert!(success);
            assert_eq!(match_index, 11);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
    assert_eq!(storage.last_log_index(), 11);
}

#[test]
fn test_safety_follower_rejects_append_with_mismatched_snapshot_term() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    let leader_entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("cmd16".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        10,
        3,
        leader_entries,
        16,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index: _,
            ..
        } => {
            assert_eq!(term, 3);
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
}

#[test]
fn test_liveness_both_nodes_compacted_replication_continues() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    assert_eq!(storage.first_log_index(), 11);
    assert_eq!(storage.last_log_index(), 20);
    let leader_entries = InMemoryLogEntryCollection::new(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd21".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd22".to_string()),
        },
    ]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        20,
        2,
        leader_entries,
        22,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index: _,
            ..
        } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
    assert_eq!(storage.last_log_index(), 22);
}

#[test]
fn test_safety_follower_rejects_inconsistent_snapshot() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let entries: Vec<LogEntry<String>> = (1..=10)
        .map(|i| LogEntry {
            term: 1,
            entry_type: raft_core::log_entry::EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let snapshot_data = state_machine.create_snapshot();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 1,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);
    storage.discard_entries_before(11);
    let leader_entries = InMemoryLogEntryCollection::new(&[LogEntry {
        term: 3,
        entry_type: EntryType::Command("cmd11".to_string()),
    }]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        3,
        10,
        2,
        leader_entries,
        11,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index: _,
            ..
        } => {
            assert_eq!(term, 3);
            assert!(!success);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
}

#[test]
fn test_liveness_leader_detects_follower_needs_snapshot() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    let peers = [0, 1];
    replication.initialize_leader_state(peers.iter().copied(), &storage);
    let mut node_collection = InMemoryNodeCollection::new();
    node_collection.push(0).unwrap();
    node_collection.push(1).unwrap();
    let config = Configuration::new(node_collection);
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    for _ in 0..15 {
        replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
            0,
            false,
            0,
            &storage,
            &mut InMemoryStateMachine::new(),
            &config,
            1,
            &mut observer,
            1,
        );
    }

    // Act
    let message = replication.get_append_entries_for_peer(0, 0, &storage);

    // Assert
    match message {
        RaftMsg::InstallSnapshot {
            term,
            leader_id: _,
            last_included_index,
            last_included_term,
            offset,
            data,
            done,
        } => {
            assert_eq!(term, 2);
            assert_eq!(last_included_index, 10);
            assert_eq!(last_included_term, 2);
            assert_eq!(offset, 0);
            assert!(!data.to_vec().is_empty());
            assert!(done);
        }
        _ => panic!("Expected InstallSnapshot, got {:?}", message),
    }
}

#[test]
fn test_liveness_follower_installs_snapshot_successfully() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let old_entries: Vec<LogEntry<String>> = (1..=5)
        .map(|i| LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("old_cmd{}", i)),
        })
        .collect();
    storage.append_entries(&old_entries);
    let mut leader_state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        leader_state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let snapshot_data = leader_state_machine.create_snapshot();
    let snapshot_bytes = snapshot_data.to_bytes().to_vec();

    // Act
    let response = replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        0,
        InMemoryChunkCollection::from_vec(snapshot_bytes),
        true,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }
    let metadata = storage.snapshot_metadata().expect("Should have snapshot");
    assert_eq!(metadata.last_included_index, 10);
    assert_eq!(metadata.last_included_term, 2);
    assert_eq!(storage.first_log_index(), 11);
    assert_eq!(state_machine.get("cmd10"), Some("value10"));
}

#[test]
fn test_safety_follower_rejects_stale_snapshot() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let current_snapshot_data = state_machine.create_snapshot();
    let current_snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 3,
        },
        data: current_snapshot_data,
    };
    storage.save_snapshot(current_snapshot);
    storage.discard_entries_before(16);
    let mut old_state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        old_state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let old_snapshot_data = old_state_machine.create_snapshot();
    let snapshot_bytes = old_snapshot_data.to_bytes().to_vec();

    // Act
    let response = replication.handle_install_snapshot(
        3,
        0,
        10,
        2,
        0,
        InMemoryChunkCollection::from_vec(snapshot_bytes),
        true,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 3);
            assert!(!success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }
    let metadata = storage.snapshot_metadata().expect("Should have snapshot");
    assert_eq!(metadata.last_included_index, 15);
}

#[test]
fn test_liveness_snapshot_transfer_with_chunks() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut leader_state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        leader_state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let snapshot_data = leader_state_machine.create_snapshot();
    let snapshot_bytes = snapshot_data.to_bytes().to_vec();
    let chunk_size = snapshot_bytes.len() / 3;
    let chunk1 = &snapshot_bytes[0..chunk_size];
    let chunk2 = &snapshot_bytes[chunk_size..chunk_size * 2];
    let chunk3 = &snapshot_bytes[chunk_size * 2..];

    // Act
    let response1 = replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        0,
        InMemoryChunkCollection::from_vec(chunk1.to_vec()),
        false,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response1 {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }

    // Act
    let response2 = replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        chunk_size as u64,
        InMemoryChunkCollection::from_vec(chunk2.to_vec()),
        false,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response2 {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }

    // Act
    let response3 = replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        (chunk_size * 2) as u64,
        InMemoryChunkCollection::from_vec(chunk3.to_vec()),
        true,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response3 {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 2);
            assert!(success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }
    let metadata = storage.snapshot_metadata().expect("Should have snapshot");
    assert_eq!(metadata.last_included_index, 10);
    assert_eq!(metadata.last_included_term, 2);
    assert_eq!(state_machine.get("cmd10"), Some("value10"));
}

#[test]
fn test_liveness_replication_resumes_after_snapshot_install() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let mut leader_state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        leader_state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let snapshot_data = leader_state_machine.create_snapshot();
    let snapshot_bytes = snapshot_data.to_bytes().to_vec();
    replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        0,
        InMemoryChunkCollection::from_vec(snapshot_bytes),
        true,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );
    let new_entries = InMemoryLogEntryCollection::new(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd11".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd12".to_string()),
        },
    ]);

    // Act
    let (response, _) = replication.handle_append_entries::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryConfigChangeCollection, InMemoryStorage, InMemoryStateMachine, NullObserver<String, InMemoryLogEntryCollection>>(
        2,
        10,
        2,
        new_entries,
        12,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
        &mut observer,
        1,
    );

    // Assert
    match response {
        RaftMsg::AppendEntriesResponse {
            term,
            success,
            match_index,
            ..
        } => {
            assert_eq!(term, 2);
            assert!(success);
            assert_eq!(match_index, 12);
        }
        _ => panic!("Expected AppendEntriesResponse"),
    }
    assert_eq!(storage.last_log_index(), 12);
}

#[test]
fn test_safety_follower_rejects_snapshot_from_old_term() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;
    let mut state_machine = InMemoryStateMachine::new();
    let mut old_state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        old_state_machine.apply(&format!("SET cmd{}=value{}", i, i));
    }
    let snapshot_data = old_state_machine.create_snapshot();
    let snapshot_bytes = snapshot_data.to_bytes().to_vec();

    // Act
    let response = replication.handle_install_snapshot(
        2,
        0,
        10,
        2,
        0,
        InMemoryChunkCollection::from_vec(snapshot_bytes),
        true,
        &mut current_term,
        &mut storage,
        &mut state_machine,
        &mut role,
    );

    // Assert
    match response {
        RaftMsg::InstallSnapshotResponse { term, success } => {
            assert_eq!(term, 3);
            assert!(!success);
        }
        _ => panic!("Expected InstallSnapshotResponse"),
    }
    assert!(storage.snapshot_metadata().is_none());
}

#[test]
fn test_liveness_leader_updates_next_index_after_snapshot_success() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("SET cmd{}=value{}", i, i));
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
    let peers = [0, 1];
    replication.initialize_leader_state(peers.iter().copied(), &storage);

    // Act
    replication.handle_install_snapshot_response(0, true, 10);

    // Assert
    match replication.get_append_entries_for_peer(0, 0, &storage) {
        RaftMsg::AppendEntries { prev_log_index, .. } => {
            assert_eq!(prev_log_index, 10);
        }
        _ => panic!("Should send AppendEntries after snapshot success"),
    }
}

#[test]
fn test_heartbeat_message() {
    // Arrange
    let mut manager = LogReplicationManager::<InMemoryMapCollection>::new();
    manager.next_index_mut().insert(2, 1);
    manager.match_index_mut().insert(2, 0);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);

    // Act
    let msg = manager
        .get_append_entries_for_peer::<String, InMemoryLogEntryCollection, InMemoryChunkCollection, InMemoryStorage>(2, 1, &storage);

    // Assert
    match msg {
        RaftMsg::AppendEntries { entries, .. } => {
            assert!(entries.is_empty());
        }
        _ => panic!("Expected AppendEntries"),
    }
}

#[test]
fn test_mark_server_as_catching_up() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Act
    replication.mark_server_catching_up(2);

    // Assert
    assert!(replication.is_catching_up(2));
    assert!(!replication.is_catching_up(3));
}

#[test]
fn test_mark_already_catching_up_server_is_idempotent() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Act
    replication.mark_server_catching_up(2);
    replication.mark_server_catching_up(2);

    // Assert
    assert!(replication.is_catching_up(2));
}

#[test]
fn test_promote_caught_up_server_when_match_index_equals_commit_index() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
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
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd4".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd5".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        2,
        true,
        3,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );
    assert_eq!(replication.commit_index(), 3);
    replication.mark_server_catching_up(4);
    replication.next_index_mut().insert(4, 1);
    replication.match_index_mut().insert(4, 0);
    assert!(replication.is_catching_up(4));

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(4, true, 2, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert!(replication.is_catching_up(4));

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(4, true, 3, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert!(!replication.is_catching_up(4));
}

#[test]
fn test_catching_up_server_does_not_affect_commit_index() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
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
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    replication.mark_server_catching_up(4);
    replication.next_index_mut().insert(4, 1);
    replication.match_index_mut().insert(4, 0);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication
        .handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(2, true, 3, &storage, &mut state_machine, &config, 1, &mut observer, 1);

    // Assert
    assert_eq!(replication.commit_index(), 3);
}

#[test]
fn test_multiple_catching_up_servers_excluded_from_quorum() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd2".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    replication.mark_server_catching_up(4);
    replication.mark_server_catching_up(5);
    replication.next_index_mut().insert(4, 1);
    replication.match_index_mut().insert(4, 0);
    replication.next_index_mut().insert(5, 1);
    replication.match_index_mut().insert(5, 0);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();

    // Act
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        4,
        true,
        2,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        5,
        true,
        2,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 0);

    // Act
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        2,
        true,
        2,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );

    // Assert
    assert_eq!(replication.commit_index(), 2);
}

#[test]
fn test_catching_up_server_promotion_via_snapshot() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    for i in 1..=12 {
        storage.append_entries(&[LogEntry {
            term: 2,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
    }
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        2,
        true,
        10,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );
    assert_eq!(replication.commit_index(), 10);
    replication.mark_server_catching_up(3);
    replication.next_index_mut().insert(3, 1);
    replication.match_index_mut().insert(3, 0);
    assert!(replication.is_catching_up(3));

    // Act
    replication.handle_install_snapshot_response(3, true, 10);

    // Assert
    assert!(!replication.is_catching_up(3));
}

#[test]
fn test_catching_up_server_not_promoted_if_behind_commit_index() {
    // Arrange
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    storage.append_entries(&[
        LogEntry {
            term: 2,
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
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd4".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("cmd5".to_string()),
        },
    ]);
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    let config = Configuration::new(peers.clone());
    replication.initialize_leader_state(peers.iter(), &storage);
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        2,
        true,
        5,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1
    );
    assert_eq!(replication.commit_index(), 5);
    replication.mark_server_catching_up(3);
    replication.next_index_mut().insert(3, 1);
    replication.match_index_mut().insert(3, 0);

    // Act
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        3,
        true,
        4,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );

    // Assert
    assert!(replication.is_catching_up(3));

    // Act
    replication.handle_append_entries_response::<String, InMemoryLogEntryCollection, InMemoryStorage, InMemoryStateMachine, InMemoryNodeCollection, InMemoryConfigChangeCollection, NullObserver<String, InMemoryLogEntryCollection>, InMemoryChunkCollection>(
        3,
        true,
        5,
        &storage,
        &mut state_machine,
        &config,
        1,
        &mut observer,
        1,
    );

    // Assert
    assert!(!replication.is_catching_up(3));
}
