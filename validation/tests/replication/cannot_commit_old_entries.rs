// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_cannot_commit_old_term_entry() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.add_node(4);
    cluster.add_node(5);
    cluster.connect_peers();

    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    cluster.partition(&[1, 2], &[3, 4, 5]);

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages_partition(&[1, 2]);

    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(1).commit_index(), 0);

    cluster.remove_node(1);
    cluster.heal_partition();
    cluster.connect_peers();

    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);

    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(4).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(5).storage().last_log_index(), 1);

    assert_eq!(cluster.get_node(2).commit_index(), 0);
    assert_eq!(cluster.get_node(3).commit_index(), 0);
    assert_eq!(cluster.get_node(4).commit_index(), 0);

    // Act
    cluster
        .get_node_mut(2)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();

    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(2).commit_index(), 2);
    assert_eq!(cluster.get_node(3).commit_index(), 2);
    assert_eq!(cluster.get_node(4).commit_index(), 2);
    assert_eq!(cluster.get_node(5).commit_index(), 2);

    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(2).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
