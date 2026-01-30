// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_leader_sends_heartbeats() {
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
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    let messages_to_2 = cluster.get_messages_from(1, 2);
    assert_eq!(messages_to_2.len(), 1);
    assert!(matches!(
        &messages_to_2[0],
        RaftMsg::AppendEntries {
            term: 1,
            entries: _,
            ..
        }
    ));

    let messages_to_3 = cluster.get_messages_from(1, 3);
    assert_eq!(messages_to_3.len(), 1);
    assert!(matches!(
        &messages_to_3[0],
        RaftMsg::AppendEntries {
            term: 1,
            entries: _,
            ..
        }
    ));
}

#[test]
fn test_liveness_heartbeat_prevents_election() {
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
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_message_from_to(1, 2);

    // Assert
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
}
