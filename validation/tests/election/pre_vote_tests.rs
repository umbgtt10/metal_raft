// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, storage::Storage, timer_service::TimerKind};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_pre_vote_transitions_to_real_election() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Act
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

#[test]
fn test_liveness_pre_vote_allows_legitimate_elections() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Act
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

#[test]
fn test_liveness_concurrent_pre_votes_eventually_elect() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

#[test]
fn test_safety_pre_vote_does_not_persist_state() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(5);

    let initial_term_all = cluster.get_node(1).current_term();
    cluster.partition_node(1);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).current_term(), initial_term_all);
    assert_eq!(cluster.get_node(2).current_term(), initial_term_all);
    assert_eq!(cluster.get_node(3).current_term(), initial_term_all);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).current_term(), initial_term_all);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
}

#[test]
fn test_safety_pre_vote_rejection_stale_log() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test".to_string()));
    cluster.deliver_messages();

    // Assert
    assert!(cluster.get_node(1).storage().last_log_index() >= 1);
    assert!(cluster.get_node(2).storage().last_log_index() >= 1);
    assert!(cluster.get_node(3).storage().last_log_index() >= 1);

    // Act
    cluster.partition_node(3);
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(3).current_term(), 1);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

#[test]
fn test_safety_pre_vote_rejection_snapshot_ahead() {
    // Arrange
    let mut cluster = TimelessTestCluster::new().with_snapshot_threshold(3);
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
    let leader_term = cluster.get_node(1).current_term();
    cluster.partition_node(3);
    for i in 1..=3 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET k{}=v{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    assert!(cluster.get_node(1).storage().load_snapshot().is_some());
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 4);

    // Act
    cluster.heal_partition();
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(3).current_term(), leader_term);
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), leader_term);
}

#[test]
fn test_safety_pre_vote_does_not_disrupt_leader() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    let leader_term = cluster.get_node(1).current_term();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test".to_string()));
    cluster.deliver_messages();
    cluster.partition_node(3);
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(3).current_term(), leader_term);
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);

    // Act
    cluster.heal_partition();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), leader_term);
}

#[test]
fn test_safety_partitioned_minority_pre_vote_fails() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    let original_term = cluster.get_node(1).current_term();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();
    cluster.partition(&[4, 5], &[1, 2, 3]);
    cluster
        .get_node_mut(4)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(4).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(4).current_term(), original_term);

    for _ in 0..3 {
        // Act
        cluster
            .get_node_mut(4)
            .on_event(Event::TimerFired(TimerKind::Election));
        cluster.deliver_messages();

        // Assert
        assert_eq!(cluster.get_node(4).current_term(), original_term);
        assert_eq!(*cluster.get_node(4).role(), NodeState::Follower);
    }

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), original_term);
}
