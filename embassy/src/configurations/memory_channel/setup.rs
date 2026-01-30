// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::client_channel_hub::ClientChannelHub;
use crate::configurations::storage::InMemoryStorage;
use crate::configurations::transport::channel::transport::{ChannelTransport, ChannelTransportHub};
use crate::embassy_node::EmbassyNode;
use crate::info;
use crate::raft_client::RaftClient;
use embassy_executor::Spawner;
use raft_core::observer::EventLevel;

pub async fn initialize_cluster(
    client_hub: &ClientChannelHub,
    spawner: Spawner,
    cancel: CancellationToken,
    observer_level: EventLevel,
) -> RaftClient {
    info!("Using Channel transport (In-Memory)");

    let transport_hub = ChannelTransportHub::new();

    for node_id in 1..=5 {
        let node_id_u64 = node_id as u64;

        let transport = transport_hub.create_transport(node_id_u64);
        let storage = InMemoryStorage::new();
        let client_rx = client_hub.get_receiver(node_id_u64);
        let node = EmbassyNode::new(node_id_u64, storage, transport, client_rx, observer_level);

        spawner
            .spawn(channel_raft_node_task(node, cancel.clone()))
            .unwrap();
    }

    RaftClient::new(client_hub.get_all_senders(), cancel)
}

#[embassy_executor::task(pool_size = 5)]
async fn channel_raft_node_task(
    node: EmbassyNode<ChannelTransport, InMemoryStorage>,
    cancel: CancellationToken,
) {
    node.run(cancel).await
}
