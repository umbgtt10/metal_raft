// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_single_node_cluster() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(1);

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
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET k=v".to_string()));
    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(1).commit_index(), 1,);
    assert_eq!(cluster.get_node(1).state_machine().get("k"), Some("v"));
}
