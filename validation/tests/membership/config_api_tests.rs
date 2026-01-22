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

// ============================================================================
// Task 3.3: Single-Server Edge Case Tests
// ============================================================================

#[test]
fn test_partition_during_config_change() {
    let mut cluster = TimelessTestCluster::with_nodes(5);

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

    let commit_before_partition = cluster.get_node(1).commit_index();

    // Initiate config change to add node 6
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(6)));

    let config_index = cluster.get_node(1).storage().last_log_index();

    // Immediately partition the network: leader (node 1) is isolated with only node 2
    // Nodes 3, 4, 5 form the other partition
    cluster.partition(&[1, 2], &[3, 4, 5]);

    // Try to replicate the config change
    cluster.deliver_messages();

    // Config change should NOT commit because we don't have majority
    // (2 out of 5 nodes is not a majority)
    assert!(
        !cluster.get_node(1).is_committed(config_index),
        "Config change should not commit without majority"
    );

    // Leader should still be able to append entries, but not commit them
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("partitioned_cmd".to_string()));
    cluster.deliver_messages();

    // Commit index should not advance
    assert_eq!(
        cluster.get_node(1).commit_index(),
        commit_before_partition,
        "Commit index should not advance during partition"
    );

    // Heal the partition
    cluster.heal_partition();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Now the config change should commit
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Config change should commit after partition heals"
    );

    // Verify configuration was applied
    assert_eq!(cluster.get_node(1).config().size(), 6);
    assert!(cluster.get_node(1).config().contains(6));
}

#[test]
fn test_snapshot_preserves_config() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add node 4 to cluster
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(4)));
    cluster.deliver_messages();

    // Verify node 4 is in configuration
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(cluster.get_node(1).config().contains(4));

    // Generate enough entries to trigger snapshot (threshold is 10 by default)
    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("cmd_{}", i)));
    }
    cluster.deliver_messages();

    // Verify node 1 has created a snapshot
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert!(snapshot.is_some(), "Node 1 should have created a snapshot");

    // The critical test: after snapshot creation, configuration should still be accessible
    // Config changes are stored in the log and survive snapshot creation
    assert_eq!(
        cluster.get_node(1).config().size(),
        4,
        "Configuration should remain accessible after snapshot creation"
    );
    assert!(
        cluster.get_node(1).config().contains(4),
        "Node 4 should still be in config after snapshot"
    );

    // Additional entries after snapshot should still work with new config
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("post_snapshot_cmd".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(1).storage().last_log_index();
    assert!(
        cluster.get_node(1).is_committed(final_index),
        "Should commit entries after snapshot with updated config"
    );
}

#[test]
fn test_remove_majority_of_servers() {
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Sequentially remove nodes 5, 4, 3 (going from 5 nodes down to 2)
    // Each removal must commit before the next can start

    // Remove node 5 (5 -> 4 nodes)
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(5)));
    cluster.deliver_messages();
    assert_eq!(cluster.get_node(1).config().size(), 4);
    assert!(!cluster.get_node(1).config().contains(5));

    // Remove node 4 (4 -> 3 nodes)
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(4)));
    cluster.deliver_messages();
    assert_eq!(cluster.get_node(1).config().size(), 3);
    assert!(!cluster.get_node(1).config().contains(4));

    // Remove node 3 (3 -> 2 nodes)
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(3)));
    cluster.deliver_messages();
    assert_eq!(cluster.get_node(1).config().size(), 2);
    assert!(!cluster.get_node(1).config().contains(3));

    // Verify the 2-node cluster can still commit
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("after_removals".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(1).storage().last_log_index();
    assert!(
        cluster.get_node(1).is_committed(final_index),
        "2-node cluster should still be able to commit"
    );

    // Verify both remaining nodes have the same log
    let node1_last = cluster.get_node(1).storage().last_log_index();
    let node2_last = cluster.get_node(2).storage().last_log_index();
    assert_eq!(
        node1_last, node2_last,
        "Remaining nodes should have identical logs"
    );
}

#[test]
fn test_config_change_with_leader_crash() {
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Initiate config change to add node 6
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::AddServer(6)));

    // Leader sends the config change entry to followers
    cluster.deliver_messages();

    // Send heartbeats to ensure full replication before crash
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    let config_index = cluster.get_node(2).storage().last_log_index();

    // Verify config change entry was replicated to at least one follower
    assert!(
        config_index > 0,
        "Config change should be replicated to follower before leader crashes"
    );

    // Save node 1's storage state before crash
    let saved_storage = cluster.get_node(1).storage().clone();

    // Leader (node 1) crashes BEFORE the config change commits
    cluster.remove_node(1);

    // Node 2 starts election
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // May need multiple election rounds
    for _ in 0..3 {
        // Check if we have a leader
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

    // Find the new leader
    let new_leader = [2, 3, 4, 5]
        .iter()
        .find(|&&node_id| *cluster.get_node(node_id).role() == NodeState::Leader)
        .copied()
        .expect("Should have elected a new leader");

    // New leader sends heartbeats to commit the config change
    for _ in 0..5 {
        cluster
            .get_node_mut(new_leader)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    // New leader should have committed the config change
    assert!(
        cluster.get_node(new_leader).is_committed(config_index),
        "New leader should commit the config change from crashed leader"
    );

    // Verify config was applied
    assert_eq!(cluster.get_node(new_leader).config().size(), 6);
    assert!(cluster.get_node(new_leader).config().contains(6));

    // Verify cluster can still make progress
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("after_crash".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(new_leader).storage().last_log_index();
    assert!(
        cluster.get_node(new_leader).is_committed(final_index),
        "Cluster should continue operating after leader crash during config change"
    );

    // Recover node 1 and verify it catches up with new config
    cluster.add_node_with_storage(1, saved_storage);
    cluster.connect_peers();

    for _ in 0..10 {
        cluster
            .get_node_mut(new_leader)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();
    }

    // Node 1 should have caught up and have the new configuration
    assert_eq!(
        cluster.get_node(1).config().size(),
        6,
        "Recovered node should have new configuration"
    );
    assert!(cluster.get_node(1).config().contains(6));
}

#[test]
fn test_leader_removes_self_and_steps_down() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Verify node 1 is the leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Leader removes itself
    cluster
        .get_node_mut(1)
        .on_event(Event::ConfigChange(ConfigurationChange::RemoveServer(1)));
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

    // Node 1 should have committed and stepped down
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Node 1 should have committed config change"
    );
    assert_eq!(
        *cluster.get_node(1).role(),
        NodeState::Follower,
        "Node 1 should have stepped down after removing itself"
    );

    // Node 1's config should reflect removal
    assert_eq!(
        cluster.get_node(1).config().size(),
        2,
        "Node 1 should have 2 nodes in config after self-removal"
    );

    // Trigger new election among remaining nodes
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Find the new leader
    let new_leader = [2, 3]
        .iter()
        .find(|&&node_id| *cluster.get_node(node_id).role() == NodeState::Leader)
        .copied()
        .expect("Should have elected a new leader from remaining nodes");

    // New leader sends a client command to commit entries from its term
    // This triggers commit of the RemoveServer entry (Raft safety: can only commit own term)
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("trigger_commit".to_string()));
    cluster.deliver_messages();

    // Propagate commit index to all nodes
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Verify configuration was updated on all nodes
    for node_id in [1, 2, 3] {
        let node = cluster.get_node(node_id);
        assert_eq!(
            node.config().size(),
            2,
            "Node {} should have 2 nodes in config",
            node_id
        );
        assert!(
            !node.config().contains(1),
            "Node {} config should not contain removed node 1",
            node_id
        );
    }

    // Verify cluster can still make progress
    cluster
        .get_node_mut(new_leader)
        .on_event(Event::ClientCommand("after_leader_removal".to_string()));
    cluster.deliver_messages();

    let final_index = cluster.get_node(new_leader).storage().last_log_index();
    assert!(
        cluster.get_node(new_leader).is_committed(final_index),
        "Cluster should continue operating after leader removes itself"
    );

    // Verify old leader (node 1) remains a Follower
    assert_eq!(
        *cluster.get_node(1).role(),
        NodeState::Follower,
        "Old leader should remain Follower"
    );
}
