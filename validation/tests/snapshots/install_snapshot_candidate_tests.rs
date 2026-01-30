// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_test_utils::in_memory_chunk_collection::InMemoryChunkCollection;
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_candidate_steps_down_on_install_snapshot_same_term() {
    // Arrange
    let mut cluster = TimelessTestCluster::with_nodes(3);
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_message_from_to(2, 1);
    cluster.deliver_message_from_to(2, 3);
    cluster.deliver_message_from_to(1, 2);
    cluster.deliver_message_from_to(3, 2);
    assert_eq!(*cluster.get_node(2).role(), NodeState::Candidate);
    let candidate_term = cluster.get_node(2).current_term();

    // Act
    let snapshot_chunk = InMemoryChunkCollection::from_vec(vec![1, 2, 3]);
    let install_snapshot = RaftMsg::InstallSnapshot {
        term: candidate_term,
        leader_id: 1,
        last_included_index: 1,
        last_included_term: candidate_term,
        offset: 0,
        data: snapshot_chunk,
        done: true,
    };
    cluster.get_node_mut(2).on_event(Event::Message {
        from: 1,
        msg: install_snapshot,
    });

    // Assert
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(2).current_term(), candidate_term);
}
