// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_state_machine_execution_order() {
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
    for i in 1..=5 {
        let cmd = format!("SET k{}=v{}", i, i);
        cluster.get_node_mut(1).on_event(Event::ClientCommand(cmd));
        cluster.deliver_messages();
    }
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    for id in 1..=3 {
        let sm = cluster.get_node(id).state_machine();
        for i in 1..=5 {
            let key = format!("k{}", i);
            let val = format!("v{}", i);
            assert_eq!(
                sm.get(&key),
                Some(val.as_str()),
                "Node {} missing key {}",
                id,
                key
            );
        }
    }

    // Act
    cluster.partition_node(1);
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);

    // Act
    for i in 6..=10 {
        let cmd = format!("SET k{}=v{}", i, i);
        cluster.get_node_mut(2).on_event(Event::ClientCommand(cmd));
        cluster.deliver_messages();
    }
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    for id in 2..=3 {
        let sm = cluster.get_node(id).state_machine();
        for i in 1..=10 {
            let key = format!("k{}", i);
            let val = format!("v{}", i);
            assert_eq!(sm.get(&key), Some(val.as_str()));
        }
    }
    let sm1 = cluster.get_node(1).state_machine();
    assert_eq!(sm1.get("k5"), Some("v5"));
    assert_eq!(sm1.get("k6"), None);

    // Act
    cluster.heal_partition();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert
    let sm1_new = cluster.get_node(1).state_machine();
    for i in 1..=10 {
        let key = format!("k{}", i);
        let val = format!("v{}", i);
        assert_eq!(
            sm1_new.get(&key),
            Some(val.as_str()),
            "Node 1 failed to catch up key {}",
            key
        );
    }
}
