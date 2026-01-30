// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, timer_service::TimerKind};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_lease_granted_on_quorum_acknowledgment() {
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

    let leader = cluster.get_node(1);
    assert_eq!(*leader.role(), NodeState::Leader);
    assert!(!leader.leader_lease().is_valid());

    // Act
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test_command".to_string()));
    cluster.deliver_messages();

    // Assert
    let leader = cluster.get_node(1);
    assert!(leader.leader_lease().is_valid());
}

#[test]
fn test_lease_revoked_on_leader_change() {
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

    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test_command".to_string()));
    cluster.deliver_messages();

    let old_leader = cluster.get_node(1);
    assert!(old_leader.leader_lease().is_valid());

    // Act
    cluster.clear_message_log();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);

    let _ = cluster.get_all_messages();

    // Assert
    let old_leader = cluster.get_node(1);
    let _ = cluster.get_node(2);

    assert_ne!(*old_leader.role(), NodeState::Leader);
    assert!(!old_leader.leader_lease().is_valid());

    cluster.clear_message_log();
    cluster
        .get_node_mut(2)
        .on_event(Event::ClientCommand("new_command".to_string()));
    cluster.deliver_messages();

    let new_leader = cluster.get_node(2);
    assert_eq!(*new_leader.role(), NodeState::Leader);
    assert!(new_leader.leader_lease().is_valid());
}

#[test]
fn test_lease_enables_linearizable_reads() {
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

    let leader = cluster.get_node_mut(1);
    assert!(leader.read_linearizable("test_key").is_err());

    // Act
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("establish_lease".to_string()));
    cluster.deliver_messages();

    // Assert
    let leader = cluster.get_node_mut(1);
    assert!(leader.read_linearizable("test_key").is_ok());
}
