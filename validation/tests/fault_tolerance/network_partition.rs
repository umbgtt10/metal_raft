// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry::EntryType, node_state::NodeState, state_machine::StateMachine,
    storage::Storage, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_network_partition_recovery() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.add_node(4);
    cluster.add_node(5);
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
    assert_eq!(cluster.get_node(5).storage().last_log_index(), 1);

    // Act
    cluster.partition(&[1, 2], &[3, 4, 5]);
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages_partition(&[1, 2]);

    // Assert
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 2);
    assert_eq!(cluster.get_node(1).commit_index(), 1);

    // Act
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages_partition(&[3, 4, 5]);

    // Assert
    assert_eq!(*cluster.get_node(3).role(), NodeState::Leader);

    // Act
    cluster
        .get_node_mut(3)
        .on_event(Event::ClientCommand("SET z=3".to_string()));
    cluster.deliver_messages_partition(&[3, 4, 5]);

    // Assert
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 2);
    assert_eq!(cluster.get_node(3).commit_index(), 2); // Committed

    // Act
    cluster.heal_partition();
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 2);
    let entry2_node1 = cluster.get_node(1).storage().get_entry(2).unwrap();
    if let EntryType::Command(ref p) = entry2_node1.entry_type {
        assert_eq!(p, "SET z=3");
    }

    for node_id in 1..=5 {
        assert_eq!(cluster.get_node(node_id).storage().last_log_index(), 2);
        assert_eq!(cluster.get_node(node_id).commit_index(), 2);

        let entry1 = cluster.get_node(node_id).storage().get_entry(1).unwrap();
        if let EntryType::Command(ref p) = entry1.entry_type {
            assert_eq!(p, "SET x=1");
        }

        let entry2 = cluster.get_node(node_id).storage().get_entry(2).unwrap();
        if let EntryType::Command(ref p) = entry2.entry_type {
            assert_eq!(p, "SET z=3");
        }
    }
    for node_id in 1..=5 {
        assert_eq!(
            cluster.get_node(node_id).state_machine().get("x"),
            Some("1")
        );
        assert_eq!(
            cluster.get_node(node_id).state_machine().get("z"),
            Some("3")
        );
        assert_eq!(cluster.get_node(node_id).state_machine().get("y"), None);
    }
}
