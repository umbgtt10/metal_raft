// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event,
    log_entry::{EntryType, LogEntry},
    node_state::NodeState,
    storage::Storage,
    timer_service::TimerKind,
};
use raft_test_utils::in_memory_storage::InMemoryStorage;
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_higher_term_beats_longer_log() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    let mut storage1 = InMemoryStorage::new();
    storage1.set_current_term(2);
    storage1.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("2".to_string()),
        },
    ]);
    cluster.add_node_with_storage(1, storage1);

    let mut storage2 = InMemoryStorage::new();
    storage2.set_current_term(1);
    storage2.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("2".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("3".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("4".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("5".to_string()),
        },
    ]);
    cluster.add_node_with_storage(2, storage2);
    cluster.connect_peers();

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 3);
    assert_eq!(cluster.get_node(2).current_term(), 3);
    assert_eq!(cluster.get_node(2).storage().voted_for(), Some(1));
}

#[test]
fn test_safety_older_longer_log_loses_to_newer_shorter() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    let mut storage1 = InMemoryStorage::new();
    storage1.set_current_term(2);
    storage1.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("1".to_string()),
        },
        LogEntry {
            term: 2,
            entry_type: EntryType::Command("2".to_string()),
        },
    ]);
    cluster.add_node_with_storage(1, storage1);

    let mut storage2 = InMemoryStorage::new();
    storage2.set_current_term(1);
    storage2.append_entries(&[
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("1".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("2".to_string()),
        },
        LogEntry {
            term: 1,
            entry_type: EntryType::Command("3".to_string()),
        },
    ]);
    cluster.add_node_with_storage(2, storage2);
    cluster.connect_peers();

    // Act
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_ne!(*cluster.get_node(2).role(), NodeState::Leader);
}
