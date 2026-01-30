// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Raft cluster handle for client interactions

use crate::cancellation_token::CancellationToken;
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::Duration;
use raft_core::types::NodeId;

// Private cluster state - not directly accessible outside this module
static CLIENT_CHANNELS: [Channel<CriticalSectionRawMutex, ClientRequest, 4>; 5] = [
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
];

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

/// Client request to Raft cluster
#[derive(Clone)]
pub enum ClientRequest {
    /// Write command to replicated log
    Write {
        payload: String,
        response_tx: Sender<'static, CriticalSectionRawMutex, Result<(), ClusterError>, 1>,
    },
    /// Read value from state machine
    Read {
        key: String,
        response_tx:
            Sender<'static, CriticalSectionRawMutex, Result<Option<String>, ClusterError>, 1>,
    },
    /// Get current leader (for client redirection)
    GetLeader,
}

/// Error types for cluster operations
#[derive(Debug, Clone, Copy)]
pub enum ClusterError {
    /// No leader currently elected (with optional hint)
    NotLeader { hint: Option<NodeId> },
    /// Channel send failed
    ChannelFull,
    /// Timeout waiting for response
    Timeout,
}

/// Handle to interact with a running Raft cluster
pub struct RaftCluster {
    /// Client channels (one per node for requests)
    client_channels: [Sender<'static, CriticalSectionRawMutex, ClientRequest, 4>; 5],

    /// For graceful shutdown
    cancel: CancellationToken,
}

impl RaftCluster {
    /// Get client channel receiver for a specific node (internal use)
    pub(crate) fn client_channel_receiver(
        node_id: NodeId,
    ) -> embassy_sync::channel::Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4> {
        let index = (node_id - 1) as usize;
        CLIENT_CHANNELS[index].receiver()
    }

    /// Get client channel sender by index (internal use)
    pub(crate) fn client_channel_sender(
        index: usize,
    ) -> embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, ClientRequest, 4> {
        CLIENT_CHANNELS[index].sender()
    }

    /// Create new cluster handle from shared statics
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            client_channels: [
                Self::client_channel_sender(0),
                Self::client_channel_sender(1),
                Self::client_channel_sender(2),
                Self::client_channel_sender(3),
                Self::client_channel_sender(4),
            ],
            cancel,
        }
    }

    /// Submit a write command to the cluster
    /// Tries nodes in sequence, following leader hints on redirects
    pub async fn submit_command(&self, payload: String) -> Result<(), ClusterError> {
        let mut target_node = 0; // Start with node 0
        let max_retries = 3;

        for attempt in 0..max_retries {
            // Clear any previous responses
            while CLIENT_WRITE_RESPONSE_CHANNEL.try_receive().is_ok() {}

            let req = ClientRequest::Write {
                payload: payload.clone(),
                response_tx: CLIENT_WRITE_RESPONSE_CHANNEL.sender(),
            };

            // Send to target node
            if self.client_channels[target_node].try_send(req).is_err() {
                return Err(ClusterError::ChannelFull);
            }

            // Wait for response
            match embassy_time::with_timeout(
                Duration::from_secs(5),
                CLIENT_WRITE_RESPONSE_CHANNEL.receive(),
            )
            .await
            {
                Ok(Ok(())) => return Ok(()), // Success!
                Ok(Err(ClusterError::NotLeader {
                    hint: Some(leader_id),
                })) => {
                    // Follow the hint
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

    /// Read a value from the state machine
    /// Tries nodes in sequence, following leader hints on redirects
    pub async fn read_value(&self, key: &str) -> Result<Option<String>, ClusterError> {
        let mut target_node = 0; // Start with node 0
        let max_retries = 3;

        for attempt in 0..max_retries {
            // Clear any previous responses
            while CLIENT_READ_RESPONSE_CHANNEL.try_receive().is_ok() {}

            let req = ClientRequest::Read {
                key: String::from(key),
                response_tx: CLIENT_READ_RESPONSE_CHANNEL.sender(),
            };

            // Send to target node
            if self.client_channels[target_node].try_send(req).is_err() {
                return Err(ClusterError::ChannelFull);
            }

            // Wait for response
            match embassy_time::with_timeout(
                Duration::from_secs(5),
                CLIENT_READ_RESPONSE_CHANNEL.receive(),
            )
            .await
            {
                Ok(Ok(value)) => return Ok(value), // Success!
                Ok(Err(ClusterError::NotLeader {
                    hint: Some(leader_id),
                })) => {
                    // Follow the hint
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

    /// Initiate graceful shutdown
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}
