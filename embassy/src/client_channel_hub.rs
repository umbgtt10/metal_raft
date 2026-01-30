// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Client channel hub for managing client-to-node communication channels

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use raft_core::types::NodeId;

use crate::raft_client::ClientRequest;

type ClientChannel = Channel<CriticalSectionRawMutex, ClientRequest, 4>;

pub type ClientReceiver =
    embassy_sync::channel::Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4>;

pub type ClientSender =
    embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, ClientRequest, 4>;

static CLIENT_CHANNELS: [ClientChannel; 5] = [
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
];

pub struct ClientChannelHub;

impl ClientChannelHub {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn get_receiver(&self, node_id: NodeId) -> ClientReceiver {
        let index = (node_id - 1) as usize;
        CLIENT_CHANNELS[index].receiver()
    }

    pub fn get_all_senders(&self) -> [ClientSender; 5] {
        [
            CLIENT_CHANNELS[0].sender(),
            CLIENT_CHANNELS[1].sender(),
            CLIENT_CHANNELS[2].sender(),
            CLIENT_CHANNELS[3].sender(),
            CLIENT_CHANNELS[4].sender(),
        ]
    }
}

impl Default for ClientChannelHub {
    fn default() -> Self {
        Self::new()
    }
}
