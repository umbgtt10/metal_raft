// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::log_entry_collection::LogEntryCollection, log_entry::EntryType,
    log_entry::LogEntry, snapshot::Snapshot, snapshot::SnapshotBuilder, snapshot::SnapshotData,
    snapshot::SnapshotMetadata, state_machine::StateMachine, storage::Storage,
};
use raft_test_utils::{
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_state_machine::InMemoryStateMachine,
    in_memory_storage::InMemoryStorage,
    snapshot_types::{SimSnapshotBuilder, SimSnapshotData},
};

#[test]
fn test_state_machine_snapshot_create_and_restore() {
    // Arrange
    let mut sm = InMemoryStateMachine::new();
    sm.apply(&"SET x=1".to_string());
    sm.apply(&"SET y=2".to_string());
    sm.apply(&"SET z=3".to_string());
    assert_eq!(sm.get("x"), Some("1"));
    assert_eq!(sm.get("y"), Some("2"));
    assert_eq!(sm.get("z"), Some("3"));

    // Act
    let snapshot_data = sm.create_snapshot();
    let mut sm2 = InMemoryStateMachine::new();
    assert_eq!(sm2.get("x"), None);
    assert_eq!(sm2.get("y"), None);
    let result = sm2.restore_from_snapshot(&snapshot_data);

    // Assert
    assert!(result.is_ok(), "Snapshot restoration should succeed");
    assert_eq!(sm2.get("x"), Some("1"));
    assert_eq!(sm2.get("y"), Some("2"));
    assert_eq!(sm2.get("z"), Some("3"));
}

#[test]
fn test_state_machine_snapshot_empty() {
    // Arrange
    let sm = InMemoryStateMachine::new();
    let snapshot_data = sm.create_snapshot();
    let mut sm2 = InMemoryStateMachine::new();
    sm2.apply(&"SET temp=value".to_string());
    assert_eq!(sm2.get("temp"), Some("value"));

    // Act
    let result = sm2.restore_from_snapshot(&snapshot_data);

    // Assert
    assert!(result.is_ok());
    assert_eq!(sm2.get("temp"), None);
}

#[test]
fn test_storage_snapshot_save_and_load() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let snapshot_data = SimSnapshotData(vec![1, 2, 3, 4, 5]);
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 5,
        },
        data: snapshot_data,
    };

    // Act
    storage.save_snapshot(snapshot.clone());
    let loaded = storage.load_snapshot();

    // Assert
    assert!(loaded.is_some(), "Snapshot should be loaded");
    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.last_included_index, 10);
    assert_eq!(loaded.metadata.last_included_term, 5);
    assert_eq!(loaded.data.0, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_storage_snapshot_metadata() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    assert!(storage.snapshot_metadata().is_none());

    // Act
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 3,
        },
        data: SimSnapshotData(vec![1, 2, 3]),
    };
    storage.save_snapshot(snapshot);
    let metadata = storage.snapshot_metadata();

    // Assert
    assert!(metadata.is_some());
    let metadata = metadata.unwrap();
    assert_eq!(metadata.last_included_index, 15);
    assert_eq!(metadata.last_included_term, 3);
}

#[test]
fn test_storage_discard_entries_before() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let entries: Vec<LogEntry<String>> = (1..=10)
        .map(|i| LogEntry {
            term: 1,
            entry_type: raft_core::log_entry::EntryType::Command(format!("entry_{}", i)),
        })
        .collect();
    storage.append_entries(&entries);
    assert_eq!(storage.first_log_index(), 1);
    assert_eq!(storage.last_log_index(), 10);
    assert!(storage.get_entry(5).is_some());

    // Act
    storage.discard_entries_before(6);

    // Assert
    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 10);
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());
    assert!(storage.get_entry(6).is_some());
    if let raft_core::log_entry::EntryType::Command(ref payload) =
        storage.get_entry(6).unwrap().entry_type
    {
        assert_eq!(payload, "entry_6");
    }
    assert!(storage.get_entry(10).is_some());
    if let raft_core::log_entry::EntryType::Command(ref payload) =
        storage.get_entry(10).unwrap().entry_type
    {
        assert_eq!(payload, "entry_10");
    }
}

#[test]
fn test_storage_discard_all_entries() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let entries: Vec<LogEntry<String>> = (1..=5)
        .map(|i| LogEntry {
            term: 1,
            entry_type: raft_core::log_entry::EntryType::Command(format!("entry_{}", i)),
        })
        .collect();
    storage.append_entries(&entries);

    // Act
    storage.discard_entries_before(6);

    // Assert
    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 5);
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());
}

#[test]
fn test_storage_get_entries_after_compaction() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let entries: Vec<LogEntry<String>> = (1..=10)
        .map(|i| LogEntry {
            term: 1,
            entry_type: EntryType::Command(format!("entry_{}", i)),
        })
        .collect();
    storage.append_entries(&entries);

    // Act
    storage.discard_entries_before(5);
    let fetched = storage.get_entries(6, 9);

    // Assert
    assert_eq!(fetched.len(), 3);
    if let EntryType::Command(ref p) = fetched.as_slice()[0].entry_type {
        assert_eq!(p, "entry_6");
    }
    if let EntryType::Command(ref p) = fetched.as_slice()[1].entry_type {
        assert_eq!(p, "entry_7");
    }
    if let EntryType::Command(ref p) = fetched.as_slice()[2].entry_type {
        assert_eq!(p, "entry_8");
    }
    let fetched = storage.get_entries(1, 5);
    assert_eq!(fetched.len(), 0);
}

#[test]
fn test_storage_last_log_term_after_compaction() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    storage.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("entry_1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("entry_2".to_string()),
        },
        LogEntry {
            term: 3,
            entry_type: EntryType::Command("entry_3".to_string()),
        },
    ]);
    assert_eq!(storage.last_log_term(), 3);

    // Act
    storage.discard_entries_before(3);
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 3,
            last_included_term: 3,
        },
        data: SimSnapshotData(vec![]),
    };
    storage.save_snapshot(snapshot);

    // Assert
    assert_eq!(storage.last_log_term(), 3);
}

#[test]
fn test_snapshot_data_chunking() {
    // Arrange
    let data = SimSnapshotData(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // Act & Assert
    let chunk = data.chunk_at(0, 3);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![1, 2, 3])
    );
    let chunk = data.chunk_at(3, 3);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![4, 5, 6])
    );
    let chunk = data.chunk_at(8, 5);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![9, 10])
    );
    let chunk = data.chunk_at(20, 5);
    assert!(chunk.is_none());
}

#[test]
fn test_storage_get_snapshot_chunk() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 5,
            last_included_term: 1,
        },
        data: SimSnapshotData(vec![1, 2, 3, 4, 5, 6, 7, 8]),
    };
    storage.save_snapshot(snapshot);

    // Act & Assert
    let chunk = storage.get_snapshot_chunk(0, 3);
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.offset, 0);
    assert_eq!(chunk.data, InMemoryChunkCollection::from_vec(vec![1, 2, 3]));
    assert!(!chunk.done);
    let chunk = storage.get_snapshot_chunk(6, 3);
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.offset, 6);
    assert_eq!(chunk.data, InMemoryChunkCollection::from_vec(vec![7, 8]));
    assert!(chunk.done);
}

#[test]
fn test_snapshot_builder_accumulate_chunks() {
    // Arrange
    let mut builder = SimSnapshotBuilder::new();

    // Act
    let result = builder.add_chunk(0, InMemoryChunkCollection::from_vec(vec![1, 2, 3]));

    // Assert
    assert!(result.is_ok());

    // Act
    let result = builder.add_chunk(3, InMemoryChunkCollection::from_vec(vec![4, 5, 6]));

    // Assert
    assert!(result.is_ok());

    // Act
    let result = builder.add_chunk(6, InMemoryChunkCollection::from_vec(vec![7, 8]));

    // Assert
    assert!(result.is_ok());
    assert!(builder.is_complete(8));
    assert_eq!(builder.build().unwrap().0, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_snapshot_builder_incomplete() {
    // Arrange
    let mut builder = SimSnapshotBuilder::new();
    builder
        .add_chunk(0, InMemoryChunkCollection::from_vec(vec![1, 2, 3]))
        .unwrap();
    builder
        .add_chunk(3, InMemoryChunkCollection::from_vec(vec![4, 5]))
        .unwrap();

    // Act
    assert!(!builder.is_complete(10));
    assert_eq!(builder.build().unwrap().0, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_storage_snapshot_transfer_workflow() {
    // Arrange
    let mut storage = InMemoryStorage::new();
    let original = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 2,
        },
        data: SimSnapshotData(vec![10, 20, 30, 40, 50]),
    };
    storage.save_snapshot(original.clone());
    let mut receiver_storage = InMemoryStorage::new();
    let mut builder = receiver_storage.begin_snapshot_transfer();

    // Act
    let chunk = storage.get_snapshot_chunk(0, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();
    let chunk = storage.get_snapshot_chunk(2, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();
    let chunk = storage.get_snapshot_chunk(4, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();
    let result = receiver_storage.finalize_snapshot(builder, original.metadata.clone());

    // Assert
    assert!(result.is_ok());
    let loaded = receiver_storage.load_snapshot();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.last_included_index, 10);
    assert_eq!(loaded.data.0, vec![10, 20, 30, 40, 50]);
}
