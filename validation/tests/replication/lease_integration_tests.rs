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

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Verify node 1 is leader and has no lease initially
    let leader = cluster.get_node(1);
    assert_eq!(*leader.role(), NodeState::Leader);
    assert!(!leader.leader_lease().is_valid());

    // Act - Submit a command to trigger replication
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test_command".to_string()));
    cluster.deliver_messages();

    // Assert - After command submission and replication, lease should be granted
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

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Submit a command to grant lease
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test_command".to_string()));
    cluster.deliver_messages();

    // Verify lease is granted
    let old_leader = cluster.get_node(1);
    assert!(old_leader.leader_lease().is_valid());

    // Force election of node 2 as new leader
    cluster.clear_message_log();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages(); // This sends RequestVote messages
    cluster.deliver_messages(); // Deliver responses to complete election
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);

    // Check what messages were sent
    let messages = cluster.get_all_messages();
    println!("Messages after node 2 election: {:?}", messages.len());

    // Check node states
    let old_leader = cluster.get_node(1);
    let _new_leader = cluster.get_node(2);

    // The old leader should have received RequestVote messages and stepped down
    assert_ne!(*old_leader.role(), NodeState::Leader);
    assert!(!old_leader.leader_lease().is_valid());

    // New leader should eventually get lease after submitting a command
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

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Initially no lease, reads should fail
    let leader = cluster.get_node_mut(1);
    assert!(leader.read_linearizable("test_key").is_err());

    // Submit command to establish lease
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("establish_lease".to_string()));
    cluster.deliver_messages();

    // Now lease should be valid and linearizable reads should succeed
    let leader = cluster.get_node_mut(1);
    assert!(leader.read_linearizable("test_key").is_ok());
}
