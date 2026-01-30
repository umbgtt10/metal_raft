// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::collections::embassy_log_collection::EmbassyLogEntryCollection;
use crate::collections::heapless_chunk_collection::HeaplessChunkVec;
use crate::configurations::transport::async_transport::AsyncTransport;
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use raft_core::raft_messages::RaftMsg;
use raft_core::types::NodeId;

#[derive(Debug, Clone)]
pub struct Envelope {
    pub from: NodeId,
    pub to: NodeId,
    pub message: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
}

const CHANNEL_SIZE: usize = 32;

pub struct ChannelTransportHub {
    channels: [Channel<CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5],
}

impl ChannelTransportHub {
    pub fn new() -> &'static Self {
        static HUB: ChannelTransportHub = ChannelTransportHub {
            channels: [
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
            ],
        };
        &HUB
    }

    pub fn create_transport(&'static self, node_id: NodeId) -> ChannelTransport {
        let rx = self.channels[(node_id - 1) as usize].receiver();
        let tx_list = self.get_all_senders();

        ChannelTransport {
            node_id,
            rx,
            tx_list,
        }
    }

    fn get_all_senders(
        &'static self,
    ) -> [Sender<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5] {
        [
            self.channels[0].sender(),
            self.channels[1].sender(),
            self.channels[2].sender(),
            self.channels[3].sender(),
            self.channels[4].sender(),
        ]
    }
}

pub struct ChannelTransport {
    node_id: NodeId,
    rx: Receiver<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>,
    tx_list: [Sender<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5],
}

impl ChannelTransport {
    pub async fn send(
        &self,
        to: NodeId,
        message: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        if to == 0 || to > 5 {
            info!("Invalid target node: {}, ignoring message", to);
            return;
        }

        let envelope = Envelope {
            from: self.node_id,
            to,
            message,
        };

        let sender = &self.tx_list[(to - 1) as usize];
        sender.send(envelope).await;
    }

    pub async fn recv(
        &mut self,
    ) -> (
        NodeId,
        RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        let envelope = self.rx.receive().await;
        (envelope.from, envelope.message)
    }

    pub async fn broadcast(
        &self,
        message: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        for peer_id in 1..=5 {
            if peer_id != self.node_id {
                self.send(peer_id, message.clone()).await;
            }
        }
    }
}

impl AsyncTransport for ChannelTransport {
    async fn send(
        &mut self,
        to: NodeId,
        message: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        ChannelTransport::send(self, to, message).await
    }

    async fn recv(
        &mut self,
    ) -> (
        NodeId,
        RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        ChannelTransport::recv(self).await
    }

    async fn broadcast(
        &mut self,
        message: RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    ) {
        ChannelTransport::broadcast(self, message).await
    }
}
