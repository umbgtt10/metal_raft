// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::node_state::NodeState;
use raft_validation::timefull_test_cluster::TimefullTestCluster;

#[test]
fn test_liveness_real_timer_triggers_election() {
    // Arrange
    let mut cluster = TimefullTestCluster::new();

    cluster.add_node_with_timeouts(1, 100, 150);
    cluster.add_node_with_timeouts(2, 10000, 10000);
    cluster.add_node_with_timeouts(3, 10000, 10000);
    cluster.connect_peers();

    // Act
    cluster.advance_time(350);
    cluster.deliver_messages();

    // Assert
    let leaders: Vec<_> = (1..=3)
        .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .collect();
    assert_eq!(leaders.len(), 1);
}

#[test]
fn test_liveness_leader_sends_heartbeats_automatically() {
    // Arrange
    let mut cluster = TimefullTestCluster::new();
    cluster.add_node_with_timeouts(1, 100, 150);
    cluster.add_node_with_timeouts(2, 10000, 10000);
    cluster.add_node_with_timeouts(3, 10000, 10000);
    cluster.connect_peers();

    // Act
    cluster.advance_time(350);
    cluster.deliver_messages();

    // Assert
    let leader_id = (1..=3)
        .find(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .expect("Should have a leader");

    // Act
    cluster.advance_time(60);
    cluster.deliver_messages();
    cluster.advance_time(300);
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(leader_id).role(), NodeState::Leader);
    let leader_count = (1..=3)
        .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .count();
    assert_eq!(leader_count, 1);
}

#[test]
fn test_liveness_split_vote_then_recovery() {
    // Arrange
    let mut cluster = TimefullTestCluster::new();
    cluster.add_node_with_timeouts(1, 100, 200);
    cluster.add_node_with_timeouts(2, 150, 250);
    cluster.add_node_with_timeouts(3, 200, 300);
    cluster.connect_peers();

    let mut elected = false;

    // Act
    for _ in 1..=5 {
        for _ in 0..15 {
            cluster.advance_time(30);
            cluster.deliver_messages();

            let leaders: Vec<_> = (1..=3)
                .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
                .collect();

            if leaders.len() == 1 {
                elected = true;
                break;
            }
        }

        if elected {
            break;
        }
    }

    // Assert
    assert!(elected);
}

#[test]
fn test_safety_election_timer_starts_without_messages() {
    // Arrange
    let mut cluster = TimefullTestCluster::new();

    // Act
    cluster.add_node_with_timeouts(1, 150, 200);

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);

    // Act
    cluster.advance_time(250);

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}
