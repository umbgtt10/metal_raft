// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        chunk_collection::ChunkCollection, configuration::Configuration,
        log_entry_collection::LogEntryCollection, node_collection::NodeCollection,
    },
    node_state::NodeState,
    raft_messages::RaftMsg,
    storage::Storage,
    timer_service::TimerService,
    types::{LogIndex, NodeId, Term},
};

pub struct ElectionManager<C, TS>
where
    C: NodeCollection,
    TS: TimerService,
{
    votes_received: C,
    pre_votes_received: C,
    in_pre_vote: bool,
    timer_service: TS,
}

impl<C, TS> ElectionManager<C, TS>
where
    C: NodeCollection,
    TS: TimerService,
{
    pub fn new(timer_service: TS) -> Self {
        Self {
            votes_received: C::new(),
            pre_votes_received: C::new(),
            in_pre_vote: false,
            timer_service,
        }
    }

    pub fn start_pre_vote<P, L, CC, S>(
        &mut self,
        node_id: NodeId,
        current_term: Term,
        storage: &S,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        self.pre_votes_received.clear();
        self.pre_votes_received.push(node_id).unwrap();
        self.in_pre_vote = true;

        self.timer_service.reset_election_timer();

        RaftMsg::PreVoteRequest {
            term: current_term,
            candidate_id: node_id,
            last_log_index: storage.last_log_index(),
            last_log_term: storage.last_log_term(),
        }
    }

    pub fn handle_pre_vote_request<P, L, CC, S>(
        &mut self,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
        current_term: Term,
        storage: &S,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        let vote_granted = if term < current_term {
            false
        } else {
            Self::is_log_up_to_date::<P, L, S>(last_log_index, last_log_term, storage)
        };

        RaftMsg::PreVoteResponse {
            term: current_term,
            vote_granted,
        }
    }

    /// Handle pre-vote response - returns true if we should start real election
    pub fn handle_pre_vote_response<NC: NodeCollection>(
        &mut self,
        from: NodeId,
        vote_granted: bool,
        config: &Configuration<NC>,
    ) -> bool {
        if !self.in_pre_vote || !vote_granted {
            return false;
        }

        self.pre_votes_received.push(from).ok();

        let pre_votes = self.pre_votes_received.len();

        // Won pre-vote if majority granted
        if config.has_quorum(pre_votes) {
            self.in_pre_vote = false;
            true // Start real election
        } else {
            false
        }
    }

    /// Start a new election - returns vote request message to broadcast
    pub fn start_election<P, L, CC, S>(
        &mut self,
        node_id: NodeId,
        current_term: &mut Term,
        storage: &mut S,
        role: &mut NodeState,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        *current_term += 1;
        storage.set_current_term(*current_term);
        storage.set_voted_for(Some(node_id));

        self.votes_received.clear();
        self.votes_received.push(node_id).unwrap(); // Vote for self
        self.in_pre_vote = false; // Clear pre-vote state

        *role = NodeState::Candidate;
        self.timer_service.reset_election_timer();

        RaftMsg::RequestVote {
            term: *current_term,
            candidate_id: node_id,
            last_log_index: storage.last_log_index(),
            last_log_term: storage.last_log_term(),
        }
    }

    /// Handle incoming vote request - returns response message
    #[allow(clippy::too_many_arguments)]
    pub fn handle_vote_request<P, L, CC, S>(
        &mut self,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
        current_term: &mut Term,
        storage: &mut S,
    ) -> RaftMsg<P, L, CC>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        if term > *current_term {
            *current_term = term;
            storage.set_current_term(term);
            storage.set_voted_for(None);
        }

        // Term validation is handled in the message handler
        let vote_granted = self.should_grant_vote(
            term,
            *current_term,
            candidate_id,
            last_log_index,
            last_log_term,
            storage,
        );

        if vote_granted {
            storage.set_voted_for(Some(candidate_id));
            self.timer_service.reset_election_timer();
        }

        RaftMsg::RequestVoteResponse {
            term: *current_term,
            vote_granted,
        }
    }

    /// Handle vote response - returns true if we should become leader
    pub fn handle_vote_response<NC: NodeCollection>(
        &mut self,
        from: NodeId,
        term: Term,
        vote_granted: bool,
        current_term: &Term,
        role: &NodeState,
        config: &Configuration<NC>,
    ) -> bool {
        if term > *current_term {
            return false;
        }

        if *role != NodeState::Candidate || term != *current_term || !vote_granted {
            return false;
        }

        self.votes_received.push(from).ok();

        let votes = self.votes_received.len();

        config.has_quorum(votes)
    }

    fn is_log_up_to_date<P, L, S>(
        candidate_last_log_index: LogIndex,
        candidate_last_log_term: Term,
        storage: &S,
    ) -> bool
    where
        L: LogEntryCollection,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        let our_term = storage.last_log_term();
        let our_index = storage.last_log_index();

        candidate_last_log_term > our_term
            || (candidate_last_log_term == our_term && candidate_last_log_index >= our_index)
    }

    fn should_grant_vote<P, L, S>(
        &self,
        term: Term,
        current_term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
        storage: &S,
    ) -> bool
    where
        L: LogEntryCollection,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        if term < current_term {
            return false;
        }

        if let Some(voted) = storage.voted_for() {
            if voted != candidate_id {
                return false;
            }
        }

        Self::is_log_up_to_date::<P, L, S>(last_log_index, last_log_term, storage)
    }

    pub fn timer_service(&self) -> &TS {
        &self.timer_service
    }

    pub fn timer_service_mut(&mut self) -> &mut TS {
        &mut self.timer_service
    }
}
