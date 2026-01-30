// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::client_channel_hub::ClientChannelHub;
use crate::configurations::storage::SemihostingStorage;
use crate::configurations::transport::udp::transport::UdpTransport;
use crate::configurations::transport::udp::udp_transport_hub::UdpTransportHub;
use crate::embassy_node::EmbassyNode;
use crate::info;
use crate::raft_client::RaftClient;
use embassy_executor::Spawner;
use raft_core::observer::EventLevel;

pub async fn initialize_client(
    client_hub: &ClientChannelHub,
    spawner: Spawner,
    cancel: CancellationToken,
    observer_level: EventLevel,
) -> RaftClient {
    info!("Using UDP transport (simulated Ethernet)");

    let udp_hub = UdpTransportHub::new();
    let stacks = udp_hub.initialize_network_stacks(spawner).await;

    for (i, stack) in stacks.into_iter().enumerate() {
        let node_id = (i + 1) as u64;

        let transport = udp_hub.create_transport_and_spawn_tasks(node_id, stack, spawner);
        let storage = SemihostingStorage::new(node_id);
        let client_rx = client_hub.get_receiver(node_id);
        let node = EmbassyNode::new(node_id, storage, transport, client_rx, observer_level);

        spawner
            .spawn(udp_raft_node_task(node, cancel.clone()))
            .unwrap();

        info!("Spawned UDP node {}", node_id);
    }

    info!("All UDP nodes started!");

    RaftClient::new(client_hub.get_all_senders(), cancel)
}

#[embassy_executor::task(pool_size = 5)]
async fn udp_raft_node_task(
    node: EmbassyNode<UdpTransport, SemihostingStorage>,
    cancel: CancellationToken,
) {
    node.run(cancel).await
}
