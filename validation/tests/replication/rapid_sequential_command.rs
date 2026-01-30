// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry::EntryType, node_state::NodeState, state_machine::StateMachine,
    storage::Storage, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_rapid_sequential_commands() {
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
    for i in 1..=20 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET x={}", i)));
        cluster.deliver_messages();
    }

    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    for node_id in 1..=3 {
        assert_eq!(cluster.get_node(node_id).storage().last_log_index(), 20);
        assert_eq!(cluster.get_node(node_id).commit_index(), 20);

        let first_log_index = cluster.get_node(node_id).storage().first_log_index();
        for i in first_log_index..=20 {
            let entry = cluster.get_node(node_id).storage().get_entry(i).unwrap();
            if let EntryType::Command(ref p) = entry.entry_type {
                assert_eq!(p, &format!("SET x={}", i));
            }
        }
    }

    assert_eq!(cluster.get_node(1).state_machine().get("x"), Some("20"));
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("20"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("20"));
}
