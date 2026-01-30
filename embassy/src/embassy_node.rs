// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use embassy_futures::select::{select4, Either4};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::Duration;
use raft_core::components::message_handler::ReadError;

use crate::cancellation_token::CancellationToken;
use crate::raft_client::{ClientRequest, ClusterError};
use crate::collections::embassy_config_change_collection::EmbassyConfigChangeCollection;
use crate::collections::embassy_log_collection::EmbassyLogEntryCollection;
use crate::collections::embassy_map_collection::EmbassyMapCollection;
use crate::collections::embassy_node_collection::EmbassyNodeCollection;
use crate::collections::heapless_chunk_collection::HeaplessChunkVec;
use crate::configurations::transport::async_transport::AsyncTransport;
use crate::configurations::transport::embassy_transport::EmbassyTransport;
use crate::embassy_observer::EmbassyObserver;
use crate::embassy_state_machine::{EmbassySnapshotData, EmbassyStateMachine};
use crate::embassy_timer::{EmbassyClock, EmbassyTimer};
use crate::led_state::LedState;

use raft_core::{
    collections::node_collection::NodeCollection,
    components::message_handler::ClientError,
    components::{
        election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    },
    event::Event,
    observer::EventLevel,
    raft_node::RaftNode,
    raft_node_builder::RaftNodeBuilder,
    storage::Storage,
    timer_service::TimerService,
    types::{LogIndex, NodeId},
};

type EmbassyRaftNode<S> = RaftNode<
    EmbassyTransport,
    S,
    String,
    EmbassyStateMachine,
    EmbassyNodeCollection,
    EmbassyLogEntryCollection,
    HeaplessChunkVec<512>,
    EmbassyMapCollection,
    EmbassyTimer,
    EmbassyObserver<String, EmbassyLogEntryCollection>,
    EmbassyConfigChangeCollection,
    EmbassyClock,
>;

pub struct EmbassyNode<
    T: AsyncTransport,
    S: Storage<
        Payload = String,
        LogEntryCollection = EmbassyLogEntryCollection,
        SnapshotData = EmbassySnapshotData,
        SnapshotChunk = HeaplessChunkVec<512>,
    >,
> {
    node_id: NodeId,
    raft_node: EmbassyRaftNode<S>,
    transport: EmbassyTransport,
    async_transport: T,
    client_rx: Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4>,
    led: LedState,
    pending_commands:
        BTreeMap<LogIndex, Sender<'static, CriticalSectionRawMutex, Result<(), ClusterError>, 1>>,
}

impl<
        T: AsyncTransport,
        S: Storage<
                Payload = String,
                LogEntryCollection = EmbassyLogEntryCollection,
                SnapshotData = EmbassySnapshotData,
                SnapshotChunk = HeaplessChunkVec<512>,
            > + Clone,
    > EmbassyNode<T, S>
{
    pub fn new(
        node_id: NodeId,
        storage: S,
        async_transport: T,
        client_rx: Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4>,
        observer_level: EventLevel,
    ) -> Self {
        info!("Node {} initializing...", node_id);
        let timer = EmbassyTimer::new();
        let transport = EmbassyTransport::new();
        let state_machine = EmbassyStateMachine::default();
        let led = LedState::new(node_id as u8);

        let mut peers = EmbassyNodeCollection::new();
        for id in 1..=5 {
            if id != node_id {
                let _ = peers.push(id);
            }
        }

        let election = ElectionManager::new(timer);
        let replication = LogReplicationManager::<EmbassyMapCollection>::new();
        let observer = EmbassyObserver::<String, EmbassyLogEntryCollection>::new(observer_level);

        let raft_node = RaftNodeBuilder::new(node_id, storage, state_machine)
            .with_election(election)
            .with_replication(replication)
            .with_transport(transport.clone(), peers, observer, EmbassyClock);

        info!("Node {} initialized as Follower", node_id);

        Self {
            node_id,
            raft_node,
            transport,
            async_transport,
            client_rx,
            led,
            pending_commands: BTreeMap::new(),
        }
    }

    pub async fn run(mut self, cancel: CancellationToken) {
        self.led.update(self.raft_node.role());

        loop {
            let cancel_future = cancel.wait();
            let client_future = self.client_rx.receive();
            let timer_future = async {
                loop {
                    let timer_service = self.raft_node.timer_service();
                    let expired_timers = timer_service.check_expired();

                    if let Some(timer_kind) = expired_timers.iter().next() {
                        return timer_kind;
                    }

                    embassy_time::Timer::after(Duration::from_millis(10)).await;
                }
            };

            let transport_future = self.async_transport.recv();

            let event = select4(cancel_future, client_future, timer_future, transport_future).await;

            match event {
                Either4::First(_) => {
                    info!("Node {} shutting down gracefully", self.node_id);
                    break;
                }
                Either4::Second(request) => {
                    self.handle_client_request(request).await;
                }
                Either4::Third(timer_kind) => {
                    self.raft_node.on_event(Event::TimerFired(timer_kind));
                    self.led.update(self.raft_node.role());
                }
                Either4::Fourth((from, msg)) => {
                    self.raft_node.on_event(Event::Message { from, msg });
                    self.led.update(self.raft_node.role());
                }
            }

            self.drain_and_send_messages().await;
            self.process_committed_requests().await;
        }

        info!("Node {} shutdown complete", self.node_id);
    }

    async fn drain_and_send_messages(&mut self) {
        for (target, msg) in self.transport.drain_outbox() {
            self.async_transport.send(target, msg).await;
        }
    }

    async fn process_committed_requests(&mut self) {
        let commit_index = self.raft_node.commit_index();
        let mut completed = Vec::new();

        for (index, _) in self.pending_commands.iter() {
            if *index <= commit_index {
                completed.push(*index);
            } else {
                // Since BTreeMap is ordered, we can stop early
                break;
            }
        }

        for index in completed {
            if let Some(tx) = self.pending_commands.remove(&index) {
                let _ = tx.try_send(Ok(()));
            }
        }
    }

    async fn handle_client_request(&mut self, request: ClientRequest) {
        match request {
            ClientRequest::Write {
                payload,
                response_tx,
            } => {
                info!("Node {} received client command: {}", self.node_id, payload);

                match self.raft_node.submit_client_command(payload.clone()) {
                    Ok(index) => {
                        self.pending_commands.insert(index, response_tx);
                    }
                    Err(ClientError::NotLeader { leader_hint }) => {
                        if let Some(leader) = leader_hint {
                            info!(
                                "Node {} not leader, suggesting Leader {}",
                                self.node_id, leader
                            );
                            let _ = response_tx
                                .try_send(Err(ClusterError::NotLeader { hint: Some(leader) }));
                        } else {
                            let _ =
                                response_tx.try_send(Err(ClusterError::NotLeader { hint: None }));
                        }
                    }
                }
            }
            ClientRequest::Read { key, response_tx } => {
                match self.raft_node.read_linearizable(&key) {
                    Ok(Some(value)) => {
                        let _ = response_tx.try_send(Ok(Some(String::from(value))));
                    }
                    Ok(None) => {
                        let _ = response_tx.try_send(Ok(None));
                    }
                    Err(ReadError::NotLeaderOrNoLease { leader_hint }) => {
                        if let Some(leader) = leader_hint {
                            info!(
                                "Node {} cannot serve read, suggesting Leader {}",
                                self.node_id, leader
                            );
                            let _ = response_tx
                                .try_send(Err(ClusterError::NotLeader { hint: Some(leader) }));
                        } else {
                            let _ =
                                response_tx.try_send(Err(ClusterError::NotLeader { hint: None }));
                        }
                    }
                }
            }
            ClientRequest::GetLeader => {
                // TODO: Respond with current leader
            }
        }
    }
}
