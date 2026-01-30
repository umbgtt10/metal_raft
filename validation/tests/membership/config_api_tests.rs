// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry::ConfigurationChange, node_state::NodeState, storage::Storage,
    timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_add_server_replicates_and_commits() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    let config_index = cluster.get_node(1).storage().last_log_index();

    cluster.deliver_messages();

    // Assert
    for node_id in [1, 2, 3] {
        assert!(cluster.get_node(node_id).storage().last_log_index() >= config_index,);
    }
    assert!(cluster.get_node(1).is_committed(config_index),);
}

#[test]
fn test_add_server_updates_configuration() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 4,);
    assert!(cluster.get_node(1).config().contains(4),);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(5)));
    let last_index = cluster.get_node(1).storage().last_log_index();

    // Assert
    assert!(last_index > 0);
}

#[test]
fn test_remove_server_replicates_and_commits() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));
    let config_index = cluster.get_node(1).storage().last_log_index();
    cluster.deliver_messages();

    // Assert
    assert!(cluster.get_node(1).is_committed(config_index),);
}

#[test]
fn test_remove_server_updates_configuration() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 2,);
    assert!(!cluster.get_node(1).config().contains(3));
    assert_eq!(cluster.get_node(1).config().quorum_size(), 2);
}

#[test]
fn test_follower_applies_committed_config_change() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(2).config().size(), 4);
    assert!(cluster.get_node(2).config().contains(4));
}

#[test]
fn test_config_change_survives_leadership_change() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(2).config().size(), 4,);
    assert!(cluster.get_node(2).config().contains(4));
}

#[test]
fn test_catching_up_server_does_not_block_commits() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("command_{}", i)));
    }
    cluster.deliver_messages();

    let commit_before_add = cluster.get_node(1).commit_index();
    assert_eq!(commit_before_add, 10);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();
    for i in 11..=20 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("command_{}", i)));
    }
    cluster.deliver_messages();

    // Assert
    let commit_after_add = cluster.get_node(1).commit_index();
    assert!(commit_after_add > commit_before_add);
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(cluster.get_node(1).config().contains(4));
}

#[test]
fn test_catching_up_server_promoted_after_catching_up() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("cmd_{}", i)));
    }
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();
    for _ in 0..10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    cluster.partition(&[1, 4], &[2, 3]);

    let commit_before_partition = cluster.get_node(1).commit_index();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("after_partition".to_string()));
    cluster.deliver_messages();

    // Assert
    let commit_after_partition = cluster.get_node(1).commit_index();
    assert_eq!(commit_before_partition, commit_after_partition);

    // Act
    cluster.heal_partition();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    let final_commit = cluster.get_node(1).commit_index();
    assert!(final_commit > commit_after_partition);
}

#[test]
fn test_partition_during_config_change() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("cmd_{}", i)));
    }
    cluster.deliver_messages();

    let commit_before_partition = cluster.get_node(1).commit_index();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(6)));

    let config_index = cluster.get_node(1).storage().last_log_index();
    cluster.partition(&[1, 2], &[3, 4, 5]);
    cluster.deliver_messages();

    // Assert
    assert!(!cluster.get_node(1).is_committed(config_index));

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("partitioned_cmd".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).commit_index(), commit_before_partition);

    // Act
    cluster.heal_partition();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    assert!(cluster.get_node(1).is_committed(config_index));
    assert_eq!(cluster.get_node(1).config().size(), 6);
    assert!(cluster.get_node(1).config().contains(6));
}

#[test]
fn test_snapshot_preserves_config() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(cluster.get_node(1).config().contains(4));

    // Act
    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("cmd_{}", i)));
    }
    cluster.deliver_messages();

    // Assert
    assert!(cluster.get_node(1).storage().load_snapshot().is_some());
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(cluster.get_node(1).config().contains(4));

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("post_snapshot_cmd".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(1).storage().last_log_index();
    assert!(cluster.get_node(1).is_committed(final_index));
}

#[test]
fn test_remove_majority_of_servers() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(5)));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(!cluster.get_node(1).config().contains(5));

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(4)));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 3);
    assert!(!cluster.get_node(1).config().contains(4));

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).config().size(), 2);
    assert!(!cluster.get_node(1).config().contains(3));

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("after_removals".to_string()));
    cluster.deliver_messages();
    let final_index = cluster.get_node(1).storage().last_log_index();

    // Assert
    assert!(cluster.get_node(1).is_committed(final_index));
    assert_eq!(
        cluster.get_node(1).storage().last_log_index(),
        cluster.get_node(2).storage().last_log_index()
    );
}

#[test]
fn test_config_change_with_leader_crash() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(6)));

    cluster.deliver_messages();

    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    let config_index = cluster.get_node(2).storage().last_log_index();

    // Assert
    assert!(
        config_index > 0,
        "Config change should be replicated to follower before leader crashes"
    );

    // Act
    let saved_storage = cluster.get_node(1).storage().clone();

    cluster.remove_node(1);

    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for _ in 0..3 {
        let has_leader = [2, 3, 4, 5]
            .iter()
            .any(|&node_id| *cluster.get_node(node_id).role() == NodeState::Leader);
        if has_leader {
            break;
        }
        cluster
            .get_node_mut(2)
            .on_event(Event::TimerFired(TimerKind::Election));
        cluster
            .get_node_mut(3)
            .on_event(Event::TimerFired(TimerKind::Election));
        cluster.deliver_messages();
    }

    let new_leader = [2, 3, 4, 5]
        .iter()
        .find(|&&node_id| *cluster.get_node(node_id).role() == NodeState::Leader)
        .copied()
        .expect("Should have elected a new leader");

    for _ in 0..5 {
        cluster
            .get_node_mut(new_leader)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    // Assert
    assert!(
        cluster.get_node(new_leader).is_committed(config_index),
        "New leader should commit the config change from crashed leader"
    );

    assert_eq!(cluster.get_node(new_leader).config().size(), 6);
    assert!(cluster.get_node(new_leader).config().contains(6));

    // Act
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("after_crash".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(new_leader).storage().last_log_index();

    // Assert
    assert!(
        cluster.get_node(new_leader).is_committed(final_index),
        "Cluster should continue operating after leader crash during config change"
    );

    // Act
    cluster.add_node_with_storage(1, saved_storage);
    cluster.connect_peers();

    for _ in 0..10 {
        cluster
            .get_node_mut(new_leader)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    // Assert
    assert_eq!(
        cluster.get_node(1).config().size(),
        6,
        "Recovered node should have new configuration"
    );
    assert!(cluster.get_node(1).config().contains(6));
}

#[test]
fn test_leader_removes_self_and_steps_down() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);

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
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(1)));
    let config_index = cluster.get_node(1).storage().last_log_index();
    cluster.deliver_messages();

    // Assert
    for node_id in [1, 2, 3] {
        assert!(cluster.get_node(node_id).storage().last_log_index() >= config_index);
    }
    assert!(cluster.get_node(1).is_committed(config_index));
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).config().size(), 2);

    // Act
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    let new_leader = [2, 3]
        .iter()
        .find(|&&node_id| *cluster.get_node(node_id).role() == NodeState::Leader)
        .copied()
        .expect("Should have elected a new leader from remaining nodes");

    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("trigger_commit".to_string()));
    cluster.deliver_messages();

    cluster
        .get_node_mut(new_leader)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    for node_id in [1, 2, 3] {
        assert_eq!(cluster.get_node(node_id).config().size(), 2);
        assert!(!cluster.get_node(node_id).config().contains(1));
    }

    // Act
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("after_leader_removal".to_string()));
    cluster.deliver_messages();

    // Assert
    let final_index = cluster.get_node(new_leader).storage().last_log_index();
    assert!(cluster.get_node(new_leader).is_committed(final_index));
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
}
