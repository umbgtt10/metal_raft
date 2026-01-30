// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::client_channel_hub::ClientChannelHub;
use crate::configurations::storage::semihosting::storage::SemihostingStorage;
use crate::configurations::transport::udp::config::{self, get_node_config};
use crate::configurations::transport::udp::driver::{MockNetDriver, NetworkBus};
use crate::configurations::transport::udp::transport::{
    self, RaftReceiver, RaftSender, UdpTransport,
};
use crate::embassy_node::EmbassyNode;
use crate::info;
use crate::raft_client::RaftClient;
use alloc::vec::Vec;
use embassy_executor::Spawner;
use raft_core::observer::EventLevel;

pub async fn initialize_cluster(
    client_hub: &ClientChannelHub,
    spawner: Spawner,
    cancel: CancellationToken,
    observer_level: EventLevel,
) -> RaftClient {
    info!("Using UDP transport (simulated Ethernet)");

    static NETWORK_BUS: NetworkBus = NetworkBus::new();

    let mut stacks = Vec::with_capacity(5);

    for node_id in 1..=5 {
        // SAFETY: Each node gets a unique NodeNetworkResources instance from
        // get_node_resources(node_id), which returns a mutable reference to a
        // distinct static resource. The resources array has 5 separate entries,
        // ensuring no aliasing occurs. The mutable reference is consumed by
        // embassy_net::new() and not accessed elsewhere during this initialization.
        let (stack, runner) = unsafe {
            let driver = MockNetDriver::new(node_id, &NETWORK_BUS);
            let config = get_node_config(node_id);
            let resources = config::get_node_resources(node_id);
            let seed = 0x0123_4567_89AB_CDEF_u64 + node_id as u64;

            embassy_net::new(driver, config, &mut resources.resources, seed)
        };

        stacks.push(stack);

        spawner.spawn(net_stack_task(node_id, runner)).unwrap();
    }

    info!("Network stacks created, waiting for configuration...");

    for (i, stack) in stacks.iter().enumerate() {
        let node_id = (i + 1) as u8;

        stack.wait_link_up().await;
        stack.wait_config_up().await;

        info!(
            "Node {} network configured: {:?}",
            node_id,
            stack.config_v4()
        );
    }

    info!("All network stacks ready!");

    static SENDER_CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
    static SENDER_CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
    static SENDER_CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
    static SENDER_CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
    static SENDER_CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

    static OUT_CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

    for (i, stack) in stacks.iter().enumerate() {
        let node_id = (i + 1) as u8;
        let node_id_u64 = node_id as u64;

        let (inbox_sender, inbox_receiver) = match node_id {
            1 => (SENDER_CHANNEL_1.sender(), SENDER_CHANNEL_1.receiver()),
            2 => (SENDER_CHANNEL_2.sender(), SENDER_CHANNEL_2.receiver()),
            3 => (SENDER_CHANNEL_3.sender(), SENDER_CHANNEL_3.receiver()),
            4 => (SENDER_CHANNEL_4.sender(), SENDER_CHANNEL_4.receiver()),
            5 => (SENDER_CHANNEL_5.sender(), SENDER_CHANNEL_5.receiver()),
            _ => unreachable!(),
        };

        let (outbox_sender, outbox_receiver) = match node_id {
            1 => (OUT_CHANNEL_1.sender(), OUT_CHANNEL_1.receiver()),
            2 => (OUT_CHANNEL_2.sender(), OUT_CHANNEL_2.receiver()),
            3 => (OUT_CHANNEL_3.sender(), OUT_CHANNEL_3.receiver()),
            4 => (OUT_CHANNEL_4.sender(), OUT_CHANNEL_4.receiver()),
            5 => (OUT_CHANNEL_5.sender(), OUT_CHANNEL_5.receiver()),
            _ => unreachable!(),
        };

        spawner
            .spawn(udp_listener_task(node_id_u64, *stack, inbox_sender))
            .unwrap();

        spawner
            .spawn(udp_sender_task(node_id_u64, *stack, outbox_receiver))
            .unwrap();

        let transport = UdpTransport::new(node_id_u64, outbox_sender, inbox_receiver);
        let storage = SemihostingStorage::new(node_id_u64);
        let client_rx = client_hub.get_receiver(node_id_u64);
        let node = EmbassyNode::new(node_id_u64, storage, transport, client_rx, observer_level);

        spawner
            .spawn(udp_raft_node_task(node, cancel.clone()))
            .unwrap();

        info!("Spawned UDP node {}", node_id);
    }

    info!("All UDP nodes started!");

    RaftClient::new(client_hub.get_all_senders(), cancel)
}

#[embassy_executor::task(pool_size = 5)]
async fn net_stack_task(_node_id: u8, mut runner: embassy_net::Runner<'static, MockNetDriver>) {
    runner.run().await
}

#[embassy_executor::task(pool_size = 5)]
async fn udp_raft_node_task(
    node: EmbassyNode<UdpTransport, SemihostingStorage>,
    cancel: CancellationToken,
) {
    node.run(cancel).await
}

#[embassy_executor::task(pool_size = 5)]
async fn udp_listener_task(node_id: u64, stack: embassy_net::Stack<'static>, sender: RaftSender) {
    if node_id > 255 {
        info!("Invalid node_id for listener: {}", node_id);
        return;
    }
    transport::run_udp_listener(node_id, stack, sender).await
}

#[embassy_executor::task(pool_size = 5)]
async fn udp_sender_task(node_id: u64, stack: embassy_net::Stack<'static>, receiver: RaftReceiver) {
    if node_id > 255 {
        info!("Invalid node_id for sender: {}", node_id);
        return;
    }
    transport::run_udp_sender(node_id, stack, receiver).await
}
