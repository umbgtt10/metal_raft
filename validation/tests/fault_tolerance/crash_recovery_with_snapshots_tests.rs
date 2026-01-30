// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_node_restarts_and_restores_from_snapshot() {
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
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert!(snapshot.is_some());
    assert_eq!(snapshot.unwrap().metadata.last_included_index, 10);
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    // Act
    let saved_storage = cluster.get_node(1).storage().clone();
    let saved_term = cluster.get_node(1).current_term();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Assert
    let recovered_node = cluster.get_node(1);
    for i in 1..=10 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }
    assert_eq!(recovered_node.storage().first_log_index(), 11);
    assert_eq!(recovered_node.current_term(), saved_term);

    let snapshot_after_restart = recovered_node.storage().load_snapshot().unwrap();
    assert_eq!(snapshot_after_restart.metadata.last_included_index, 10);
}

#[test]
fn test_restart_with_snapshot_and_remaining_entries() {
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

    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    let snapshot = cluster.get_node(1).storage().load_snapshot().unwrap();
    assert_eq!(snapshot.metadata.last_included_index, 10);
    assert_eq!(cluster.get_node(1).commit_index(), 15);

    // Act
    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Assert
    let recovered_node = cluster.get_node(1);
    for i in 1..=10 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    for i in 11..=15 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            None
        );
    }

    assert_eq!(recovered_node.storage().first_log_index(), 11);
    assert_eq!(recovered_node.storage().last_log_index(), 15);

    for i in 11..=15 {
        assert!(recovered_node.storage().get_entry(i).is_some());
    }

    for i in 1..=10 {
        assert!(recovered_node.storage().get_entry(i).is_none());
    }
}

#[test]
fn test_follower_crash_during_snapshot_transfer() {
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

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    assert!(cluster.get_node(1).storage().load_snapshot().is_some());

    // Act
    let saved_storage = cluster.get_node(2).storage().clone();
    cluster.remove_node(2);

    for i in 11..=12 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    cluster.add_node_with_storage(2, saved_storage);
    cluster.reconnect_node(2);
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
}

#[test]
fn test_node_restart_without_snapshot() {
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

    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Assert
    assert!(cluster.get_node(1).storage().load_snapshot().is_none());

    // Act
    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Assert
    let recovered_node = cluster.get_node(1);

    for i in 1..=5 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            None
        );
    }

    assert_eq!(recovered_node.storage().last_log_index(), 5);
    assert_eq!(recovered_node.storage().first_log_index(), 1);
}

#[test]
fn test_multiple_restart_cycles() {
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

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!(
                "SET cycle1_key{}=value{}",
                i, i
            )));
        cluster.deliver_messages();
    }

    let storage1 = cluster.get_node(1).storage().clone();

    cluster.remove_node(1);
    cluster.add_node_with_storage(1, storage1);

    // Assert
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle1_key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    // Act
    cluster.reconnect_node(1);
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!(
                "SET cycle2_key{}=value{}",
                i, i
            )));
        cluster.deliver_messages();
    }

    let storage2 = cluster.get_node(1).storage().clone();

    cluster.remove_node(1);
    cluster.add_node_with_storage(1, storage2);

    // Assert
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle1_key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    for i in 1..=5 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle2_key{}", i)),
            None
        );
    }
}

#[test]
fn test_recovered_node_can_continue_operation() {
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

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    cluster.reconnect_node(1);
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET new_key=new_value".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(
        cluster.get_node(1).state_machine().get("new_key"),
        Some("new_value")
    );

    // Assert
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }
}
