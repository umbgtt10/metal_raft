// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_test_utils::in_memory_storage::InMemoryStorage;
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_follower_far_behind() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
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
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET x={}", i)));
        cluster.deliver_messages();
    }

    // Assert
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 5);

    // Act
    let mut storage_node2 = InMemoryStorage::new();
    for i in 1..=2 {
        if let Some(entry) = cluster.get_node(1).storage().get_entry(i) {
            storage_node2.append_entries(&[entry]);
        }
    }
    cluster.remove_node(2);
    cluster.add_node_with_storage(2, storage_node2);
    cluster.connect_peers();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("noop".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 6);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);

    // Act
    for _ in 0..10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();

        if cluster.get_node(2).storage().last_log_index() == 6 {
            break;
        }
    }

    // Assert
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 6);
    for i in 1..=5 {
        let entry = cluster.get_node(2).storage().get_entry(i).unwrap();
        if let raft_core::log_entry::EntryType::Command(ref p) = entry.entry_type {
            assert_eq!(p, &format!("SET x={}", i));
        }
    }
    assert_eq!(cluster.get_node(2).commit_index(), 6);
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("5"));
}
