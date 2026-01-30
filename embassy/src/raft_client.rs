// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Raft client for interacting with a Raft cluster

use crate::cancellation_token::CancellationToken;
use crate::client_channel_hub::ClientSender;
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::Duration;
use raft_core::types::NodeId;

static CLIENT_WRITE_RESPONSE_CHANNEL: Channel<
    CriticalSectionRawMutex,
    Result<(), ClusterError>,
    1,
> = Channel::new();

static CLIENT_READ_RESPONSE_CHANNEL: Channel<
    CriticalSectionRawMutex,
    Result<Option<String>, ClusterError>,
    1,
> = Channel::new();

#[derive(Clone)]
pub enum ClientRequest {
    Write {
        payload: String,
        response_tx: Sender<'static, CriticalSectionRawMutex, Result<(), ClusterError>, 1>,
    },
    Read {
        key: String,
        response_tx:
            Sender<'static, CriticalSectionRawMutex, Result<Option<String>, ClusterError>, 1>,
    },
    GetLeader,
}

#[derive(Debug, Clone, Copy)]
pub enum ClusterError {
    NotLeader { hint: Option<NodeId> },
    ChannelFull,
    Timeout,
}

pub struct RaftClient {
    client_channels: [ClientSender; 5],
    cancel: CancellationToken,
}

impl RaftClient {
    pub fn new(client_channels: [ClientSender; 5], cancel: CancellationToken) -> Self {
        Self {
            client_channels,
            cancel,
        }
    }

    pub async fn submit_command(&self, payload: String) -> Result<(), ClusterError> {
        let mut target_node = 0;
        let max_retries = 3;

        for attempt in 0..max_retries {
            while CLIENT_WRITE_RESPONSE_CHANNEL.try_receive().is_ok() {}

            let req = ClientRequest::Write {
                payload: payload.clone(),
                response_tx: CLIENT_WRITE_RESPONSE_CHANNEL.sender(),
            };

            if self.client_channels[target_node].try_send(req).is_err() {
                return Err(ClusterError::ChannelFull);
            }

            match embassy_time::with_timeout(
                Duration::from_secs(5),
                CLIENT_WRITE_RESPONSE_CHANNEL.receive(),
            )
            .await
            {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(ClusterError::NotLeader {
                    hint: Some(leader_id),
                })) => {
                    target_node = (leader_id - 1) as usize;
                    if attempt < max_retries - 1 {
                        embassy_time::Timer::after(Duration::from_millis(50)).await;
                    }
                }
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(ClusterError::Timeout),
            }
        }

        Err(ClusterError::NotLeader { hint: None })
    }

    pub async fn read_value(&self, key: &str) -> Result<Option<String>, ClusterError> {
        let mut target_node = 0;
        let max_retries = 3;

        for attempt in 0..max_retries {
            while CLIENT_READ_RESPONSE_CHANNEL.try_receive().is_ok() {}

            let req = ClientRequest::Read {
                key: String::from(key),
                response_tx: CLIENT_READ_RESPONSE_CHANNEL.sender(),
            };

            if self.client_channels[target_node].try_send(req).is_err() {
                return Err(ClusterError::ChannelFull);
            }

            match embassy_time::with_timeout(
                Duration::from_secs(5),
                CLIENT_READ_RESPONSE_CHANNEL.receive(),
            )
            .await
            {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(ClusterError::NotLeader {
                    hint: Some(leader_id),
                })) => {
                    target_node = (leader_id - 1) as usize;
                    if attempt < max_retries - 1 {
                        embassy_time::Timer::after(Duration::from_millis(50)).await;
                    }
                }
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(ClusterError::Timeout),
            }
        }

        Err(ClusterError::NotLeader { hint: None })
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}
