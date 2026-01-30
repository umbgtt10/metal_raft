// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, timer_service::TimerKind};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_vote_rejection_already_voted() {
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
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Act
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).current_term(), 2);
}
