// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, state_machine::StateMachine, timer_service::TimerKind};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_state_machine_apply() {
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

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(1).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(2).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
