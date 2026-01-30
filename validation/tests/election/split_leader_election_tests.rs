// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_validation::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_split_vote_no_leader() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.add_node(4);
    cluster.connect_peers();

    let expected_pre_vote_from_1 = RaftMsg::PreVoteRequest {
        term: 0,
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    let expected_pre_vote_from_2 = RaftMsg::PreVoteRequest {
        term: 0,
        candidate_id: 2,
        last_log_index: 0,
        last_log_term: 0,
    };

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_message_from_to(1, 3);
    cluster.deliver_message_from_to(2, 4);

    // Assert
    assert_eq!(cluster.get_messages(3), vec![expected_pre_vote_from_1]);
    assert_eq!(cluster.get_messages(4), vec![expected_pre_vote_from_2]);

    // Act
    cluster.deliver_messages();

    // Assert
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).current_term(), 1);
    assert_eq!(cluster.get_node(2).current_term(), 1);
    let all_messages = cluster.get_all_messages();
    assert_eq!(
        all_messages
            .iter()
            .filter(|(_, _, msg)| {
                matches!(
                    msg,
                    RaftMsg::RequestVote {
                        term: 1,
                        candidate_id: 1,
                        last_log_index: 0,
                        last_log_term: 0,
                        ..
                    }
                )
            })
            .count(),
        3
    );

    assert_eq!(
        all_messages
            .iter()
            .filter(|(_, _, msg)| {
                matches!(
                    msg,
                    RaftMsg::RequestVote {
                        term: 1,
                        candidate_id: 2,
                        last_log_index: 0,
                        last_log_term: 0,
                        ..
                    }
                )
            })
            .count(),
        3
    );

    assert_eq!(
        all_messages
            .iter()
            .filter(|(_, _, msg)| {
                matches!(
                    msg,
                    RaftMsg::RequestVoteResponse {
                        vote_granted: true,
                        ..
                    }
                )
            })
            .count(),
        2
    );

    assert_eq!(
        all_messages
            .iter()
            .filter(|(_, _, msg)| {
                matches!(
                    msg,
                    RaftMsg::RequestVoteResponse {
                        vote_granted: false,
                        ..
                    }
                )
            })
            .count(),
        4
    );
}
