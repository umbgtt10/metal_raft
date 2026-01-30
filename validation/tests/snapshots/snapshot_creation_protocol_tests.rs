// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_test_utils::in_memory_state_machine::InMemoryStateMachine;
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_leader_creates_snapshot_after_threshold() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    assert_eq!(cluster.get_node(1).commit_index(), 10);
    let snapshot = cluster.get_node(1).storage().load_snapshot().unwrap();
    assert_eq!(snapshot.metadata.last_included_index, 10);
    assert_eq!(snapshot.metadata.last_included_term, 1);

    // Act
    let mut test_sm = InMemoryStateMachine::new();
    let restore_result = test_sm.restore_from_snapshot(&snapshot.data);

    // Assert
    assert!(restore_result.is_ok());
    for i in 1..=10 {
        let expected = format!("value{}", i);
        assert_eq!(test_sm.get(&format!("key{}", i)), Some(&expected[..]));
    }
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 10);
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 11);
}

#[test]
fn test_leader_automatically_compacts_log_at_threshold() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    assert_eq!(cluster.get_node(1).commit_index(), 15);
    let snapshot = cluster.get_node(1).storage().load_snapshot().unwrap();
    assert_eq!(snapshot.metadata.last_included_index, 10);
    assert_eq!(snapshot.metadata.last_included_term, 1);
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 11);
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 15);
    assert!(cluster.get_node(1).storage().get_entry(11).is_some());
    assert!(cluster.get_node(1).storage().get_entry(5).is_none());
    let state_machine = cluster.get_node(1).state_machine();
    for i in 1..=15 {
        let expected = format!("value{}", i);
        assert_eq!(state_machine.get(&format!("key{}", i)), Some(&expected[..]));
    }

    // Act
    let mut test_sm = InMemoryStateMachine::new();
    let restore_result = test_sm.restore_from_snapshot(&snapshot.data);

    // Assert
    assert!(restore_result.is_ok());
    for i in 1..=10 {
        let expected = format!("value{}", i);
        assert_eq!(test_sm.get(&format!("key{}", i)), Some(&expected[..]));
    }

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET key16=value16".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).commit_index(), 16);
    assert_eq!(
        cluster.get_node(1).state_machine().get("key16"),
        Some("value16")
    );
}
