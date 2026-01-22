// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry::ConfigurationChange, storage::Storage, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

// Note: Basic add_server/remove_server validation tests have been moved to message_handler_tests.rs
// These tests focus on integration testing with the full cluster and event system.

// ============================================================================
// Integration Tests - Actual Configuration Changes
// ============================================================================

#[test]
fn test_add_server_replicates_and_commits() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Leader submits add_server via Event
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    let config_index = cluster.get_node(1).storage().last_log_index();

    // Replicate to followers
    cluster.deliver_messages();

    // Verify the entry is in all nodes' logs
    for node_id in [1, 2, 3] {
        let node = cluster.get_node(node_id);
        assert!(
            node.storage().last_log_index() >= config_index,
            "Node {} should have config entry",
            node_id
        );
    }

    // Leader should commit once majority has replicated
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Config change should be committed"
    );
}

#[test]
fn test_add_server_updates_configuration() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Verify initial config has 3 nodes (1 self + 2 peers)
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Add node 4
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));

    // Replicate and commit
    cluster.deliver_messages();

    // Verify leader has new config (1 self + 3 peers)
    assert_eq!(
        cluster.get_node(1).config().size(),
        4,
        "Leader should have 4 members after commit"
    );
    assert!(
        cluster.get_node(1).config().contains(4),
        "Config should contain node 4"
    );

    // Verify pending flag is cleared after commit
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(5)));
    let last_index = cluster.get_node(1).storage().last_log_index();
    assert!(
        last_index > 0,
        "Should allow another config change after first commits"
    );
}

#[test]
fn test_remove_server_replicates_and_commits() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Remove node 3
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));
    let config_index = cluster.get_node(1).storage().last_log_index();

    // Replicate to followers
    cluster.deliver_messages();

    // Verify committed on leader
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Config change should be committed"
    );
}

#[test]
fn test_remove_server_updates_configuration() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Verify initial config has 3 nodes (1 self + 2 peers)
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Remove node 3
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));

    // Replicate and commit
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Verify leader has new config (1 self + 1 peer)
    assert_eq!(
        cluster.get_node(1).config().size(),
        2,
        "Leader should have 2 members after commit"
    );
    assert!(
        !cluster.get_node(1).config().contains(3),
        "Config should not contain node 3"
    );

    // Verify quorum calculation updated (2 nodes, quorum = 2)
    assert_eq!(cluster.get_node(1).config().quorum_size(), 2);
}

#[test]
fn test_follower_applies_committed_config_change() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add node 4 on leader
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));

    // Replicate to followers
    cluster.deliver_messages();
    // Send heartbeat to propagate commit index
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();
    // Verify follower node 2 also has new config
    assert_eq!(
        cluster.get_node(2).config().size(),
        4,
        "Follower should have 4 members"
    );
    assert!(
        cluster.get_node(2).config().contains(4),
        "Follower should have node 4 in config"
    );
}

#[test]
fn test_config_change_survives_leadership_change() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add node 4
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));

    // Replicate and commit
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Node 2 becomes new leader
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // New leader should still have the config with 4 nodes
    assert_eq!(
        cluster.get_node(2).config().size(),
        4,
        "New leader should have committed config"
    );
    assert!(cluster.get_node(2).config().contains(4));
}

#[test]
fn test_catching_up_server_does_not_block_commits() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Replicate some entries to establish a baseline
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("command_{}", i)));
    }
    cluster.deliver_messages();

    let commit_before_add = cluster.get_node(1).commit_index();
    assert_eq!(
        commit_before_add, 10,
        "Should have committed 10 entries before adding server"
    );

    // Add node 4 (which will be marked as catching up)
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();

    // Send more commands - these should commit even though node 4 hasn't caught up yet
    for i in 11..=20 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("command_{}", i)));
    }
    cluster.deliver_messages();

    // Verify that commits can proceed with just nodes 1, 2, 3 (majority of voting members)
    // Node 4 is catching up and not counted in quorum
    let commit_after_add = cluster.get_node(1).commit_index();
    assert!(
        commit_after_add > commit_before_add,
        "Should be able to commit new entries while node 4 is catching up. \
         Commit before: {}, after: {}",
        commit_before_add,
        commit_after_add
    );

    // Verify node 4 is indeed in the configuration
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(cluster.get_node(1).config().contains(4));
}

#[test]
fn test_catching_up_server_promoted_after_catching_up() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Establish some log history
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("cmd_{}", i)));
    }
    cluster.deliver_messages();

    // Add node 4 (starts catching up)
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();

    // Send entries to node 4 until it catches up
    for _ in 0..10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    // At this point, node 4 should have caught up (match_index >= commit_index)
    // and should be participating in quorum

    // Now if we partition nodes 2 and 3, commits should still fail
    // because we'd only have nodes 1 and 4 (not a majority of all 4 nodes)
    cluster.partition(&[1, 4], &[2, 3]);

    let commit_before_partition = cluster.get_node(1).commit_index();

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("after_partition".to_string()));
    cluster.deliver_messages();

    // Should not commit with just 2 out of 4 nodes
    let commit_after_partition = cluster.get_node(1).commit_index();
    assert_eq!(
        commit_before_partition, commit_after_partition,
        "Should not commit with minority (2/4 nodes)"
    );

    // Heal partition
    cluster.heal_partition();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Now commits should succeed again
    let final_commit = cluster.get_node(1).commit_index();
    assert!(
        final_commit > commit_after_partition,
        "Should commit after healing partition. Before: {}, After: {}",
        commit_after_partition,
        final_commit
    );
}
