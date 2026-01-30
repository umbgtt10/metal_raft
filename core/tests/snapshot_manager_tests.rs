// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    components::snapshot_manager::SnapshotManager,
    log_entry::{EntryType, LogEntry},
    snapshot::{Snapshot, SnapshotError, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
};
use raft_test_utils::{
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
};

#[test]
fn test_new_manager() {
    // Arrange
    let manager = SnapshotManager::new(100);

    // Act & Assert
    assert_eq!(manager.threshold(), 100);
}

#[test]
fn test_threshold_getter() {
    // Arrange
    let manager = SnapshotManager::new(50);

    // Act & Assert
    assert_eq!(manager.threshold(), 50);
}

#[test]
fn test_should_create_below_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(!manager.should_create(50, &storage));
    assert!(!manager.should_create(99, &storage));
}

#[test]
fn test_should_create_at_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(manager.should_create(100, &storage));
}

#[test]
fn test_should_create_above_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(manager.should_create(150, &storage));
    assert!(manager.should_create(200, &storage));
}

#[test]
fn test_should_create_with_existing_snapshot_below_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    state_machine.apply(&"SET key=value".to_string());
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 50,
            last_included_term: 1,
        },
        data: state_machine.create_snapshot(),
    };

    // Act
    storage.save_snapshot(snapshot);

    // Assert
    assert!(manager.should_create(120, &storage));
}

#[test]
fn test_should_create_with_existing_snapshot_at_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    state_machine.apply(&"SET key=value".to_string());
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 100,
            last_included_term: 1,
        },
        data: state_machine.create_snapshot(),
    };

    // Act
    storage.save_snapshot(snapshot);

    // Assert
    assert!(!manager.should_create(120, &storage));
}

#[test]
fn test_should_create_with_existing_snapshot_above_threshold() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    state_machine.apply(&"SET key=value".to_string());
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 150,
            last_included_term: 1,
        },
        data: state_machine.create_snapshot(),
    };

    // Act
    storage.save_snapshot(snapshot);

    // Assert
    assert!(!manager.should_create(200, &storage));
}

#[test]
fn test_should_create_zero_commit_index() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(!manager.should_create(0, &storage));
}

#[test]
fn test_create_snapshot_success() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("SET key1=value1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("SET key2=value2".to_string()),
        },
    ]);
    state_machine.apply(&"SET key1=value1".to_string());
    state_machine.apply(&"SET key2=value2".to_string());

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 2);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2);
    let snapshot = storage.load_snapshot().expect("Snapshot should exist");
    assert_eq!(snapshot.metadata.last_included_index, 2);
    assert_eq!(snapshot.metadata.last_included_term, 1);
}

#[test]
fn test_create_snapshot_zero_last_applied() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 0);

    // Assert
    assert_eq!(result, Err(SnapshotError::NoEntriesToSnapshot));
}

#[test]
fn test_create_snapshot_missing_entry() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 10);

    // Assert
    assert_eq!(result, Err(SnapshotError::EntryNotFound));
}

#[test]
fn test_create_snapshot_compacts_log() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
        state_machine.apply(&format!("SET key{}=value{}", i, i));
    }
    assert_eq!(storage.first_log_index(), 1);
    assert_eq!(storage.last_log_index(), 10);

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 5);

    // Assert
    assert!(result.is_ok());
    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 10);
    assert!(storage.get_entry(5).is_none());
    assert!(storage.get_entry(4).is_none());
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(6).is_some());
    assert!(storage.get_entry(10).is_some());
}

#[test]
fn test_create_snapshot_preserves_state_machine() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("SET key1=value1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("SET key2=value2".to_string()),
        },
    ]);
    state_machine.apply(&"SET key1=value1".to_string());
    state_machine.apply(&"SET key2=value2".to_string());

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 2);

    // Assert
    assert!(result.is_ok());
    let snapshot = storage.load_snapshot().unwrap();
    let mut restored_state_machine = InMemoryStateMachine::new();
    restored_state_machine
        .restore_from_snapshot(&snapshot.data)
        .unwrap();
    assert_eq!(restored_state_machine.get("key1"), Some("value1"));
    assert_eq!(restored_state_machine.get("key2"), Some("value2"));
}

#[test]
fn test_compact_removes_entries_before_index() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    for i in 1..=10 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
    }
    assert_eq!(storage.first_log_index(), 1);
    assert_eq!(storage.last_log_index(), 10);

    // Act
    manager.compact(&mut storage, 5);

    // Assert
    assert_eq!(storage.first_log_index(), 6);
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());
    assert!(storage.get_entry(6).is_some());
    assert!(storage.get_entry(10).is_some());
}

#[test]
fn test_compact_all_entries() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();
    for i in 1..=5 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("cmd{}", i)),
        }]);
    }

    // Act
    manager.compact(&mut storage, 5);

    // Assert
    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 5);
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());
}

#[test]
fn test_compact_empty_log() {
    // Arrange
    let manager = SnapshotManager::new(100);
    let mut storage = InMemoryStorage::new();

    // Act
    manager.compact(&mut storage, 10);

    // Assert
    assert_eq!(storage.first_log_index(), 11);
    assert_eq!(storage.last_log_index(), 10);
}

#[test]
fn test_snapshot_workflow() {
    // Arrange
    let manager = SnapshotManager::new(5);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();

    // Act & Assert
    assert!(!manager.should_create(4, &storage));
    for i in 1..=10 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("SET key{}=value{}", i, i)),
        }]);
        state_machine.apply(&format!("SET key{}=value{}", i, i));
    }
    assert!(manager.should_create(5, &storage));

    // Act
    let result = manager.create(&mut storage, &mut state_machine, 5);
    assert!(result.is_ok());

    // Assert
    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(snapshot.metadata.last_included_index, 5);
    assert_eq!(storage.first_log_index(), 6);
    assert!(!manager.should_create(7, &storage));

    // Act
    for i in 11..=20 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("SET key{}=value{}", i, i)),
        }]);
        state_machine.apply(&format!("SET key{}=value{}", i, i));
    }

    // Assert
    assert!(!manager.should_create(15, &storage));
}

#[test]
fn test_multiple_snapshots_in_sequence() {
    // Arrange
    let manager = SnapshotManager::new(10);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=30 {
        storage.append_entries(&[LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("SET key{}=value{}", i, i)),
        }]);
        state_machine.apply(&format!("SET key{}=value{}", i, i));
    }

    // Act
    let result1 = manager.create(&mut storage, &mut state_machine, 10);
    let result2 = manager.create(&mut storage, &mut state_machine, 20);
    let result3 = manager.create(&mut storage, &mut state_machine, 30);

    // Assert
    assert!(result1.is_ok());
    assert_eq!(storage.first_log_index(), 31);
    assert!(result2.is_ok());
    assert!(result3.is_ok());
    let final_snapshot = storage.load_snapshot().unwrap();
    assert_eq!(final_snapshot.metadata.last_included_index, 30);
}

#[test]
fn test_threshold_zero() {
    // Arrange
    let manager = SnapshotManager::new(0);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(manager.should_create(0, &storage));
    assert!(manager.should_create(100, &storage));
    assert!(manager.should_create(1000, &storage));
}

#[test]
fn test_threshold_one() {
    // Arrange
    let manager = SnapshotManager::new(1);
    let storage = InMemoryStorage::new();

    // Act & Assert
    assert!(!manager.should_create(0, &storage));
    assert!(manager.should_create(1, &storage));
    assert!(manager.should_create(2, &storage));
}
