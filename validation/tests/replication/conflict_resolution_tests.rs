// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event,
    log_entry::{EntryType, LogEntry},
    node_state::NodeState,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerKind,
};
use raft_test_utils::in_memory_storage::InMemoryStorage;
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_log_conflict_resolution() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(3);
    cluster.connect_peers();

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 1);

    // Act
    let mut storage_node2 = InMemoryStorage::new();
    storage_node2.append_entries(&[LogEntry {
        term: 1,
        entry_type: raft_core::log_entry::EntryType::Command("SET x=1".to_string()),
    }]);
    storage_node2.append_entries(&[LogEntry {
        term: 1,
        entry_type: raft_core::log_entry::EntryType::Command("SET x=99".to_string()),
    }]);
    cluster.add_node_with_storage(2, storage_node2);
    cluster.connect_peers();

    // Assert
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);

    // Act
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(3).role(), NodeState::Leader);
    cluster
        .get_node_mut(3)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);
    if let EntryType::Command(ref p) = cluster
        .get_node(2)
        .storage()
        .get_entry(2)
        .unwrap()
        .entry_type
    {
        assert_eq!(p, "SET y=2");
    } else {
        panic!("Expected Command entry");
    }
    if let EntryType::Command(ref p) = cluster
        .get_node(3)
        .storage()
        .get_entry(2)
        .unwrap()
        .entry_type
    {
        assert_eq!(p, "SET y=2");
    } else {
        panic!("Expected Command entry");
    }

    // Act
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(2).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
