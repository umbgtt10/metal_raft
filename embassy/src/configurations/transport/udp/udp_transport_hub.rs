// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::config::{self, get_node_config};
use super::driver::{MockNetDriver, NetworkBus};
use super::transport::{self, RaftReceiver, RaftSender, UdpTransport};
use crate::info;
use alloc::vec::Vec;
use embassy_executor::Spawner;

pub struct UdpTransportHub;

static NETWORK_BUS: NetworkBus = NetworkBus::new();

static INBOX_CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
static INBOX_CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
static INBOX_CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
static INBOX_CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
static INBOX_CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

static OUTBOX_CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
static OUTBOX_CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
static OUTBOX_CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
static OUTBOX_CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
static OUTBOX_CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

impl UdpTransportHub {
    pub const fn new() -> Self {
        Self {}
    }

    pub async fn initialize_network_stacks(
        &self,
        spawner: Spawner,
    ) -> Vec<embassy_net::Stack<'static>> {
        let mut stacks = Vec::with_capacity(5);

        for node_id in 1..=5 {
            // SAFETY: Each node gets a unique NodeNetworkResources instance from
            // get_node_resources(node_id). The loop ensures node_id is 1-5, so each
            // call accesses a distinct static resource without aliasing.
            let stack = unsafe { Self::create_network_stack_and_spawn_runner(node_id, spawner) };

            stacks.push(stack);
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

        stacks
    }

    pub fn create_transport_and_spawn_tasks(
        &self,
        node_id: u64,
        stack: embassy_net::Stack<'static>,
        spawner: Spawner,
    ) -> UdpTransport {
        let node_id_u8 = node_id as u8;

        let (inbox_sender, inbox_receiver) = match node_id_u8 {
            1 => (INBOX_CHANNEL_1.sender(), INBOX_CHANNEL_1.receiver()),
            2 => (INBOX_CHANNEL_2.sender(), INBOX_CHANNEL_2.receiver()),
            3 => (INBOX_CHANNEL_3.sender(), INBOX_CHANNEL_3.receiver()),
            4 => (INBOX_CHANNEL_4.sender(), INBOX_CHANNEL_4.receiver()),
            5 => (INBOX_CHANNEL_5.sender(), INBOX_CHANNEL_5.receiver()),
            _ => panic!("Invalid node_id: {}", node_id),
        };

        let (outbox_sender, outbox_receiver) = match node_id_u8 {
            1 => (OUTBOX_CHANNEL_1.sender(), OUTBOX_CHANNEL_1.receiver()),
            2 => (OUTBOX_CHANNEL_2.sender(), OUTBOX_CHANNEL_2.receiver()),
            3 => (OUTBOX_CHANNEL_3.sender(), OUTBOX_CHANNEL_3.receiver()),
            4 => (OUTBOX_CHANNEL_4.sender(), OUTBOX_CHANNEL_4.receiver()),
            5 => (OUTBOX_CHANNEL_5.sender(), OUTBOX_CHANNEL_5.receiver()),
            _ => panic!("Invalid node_id: {}", node_id),
        };

        spawner
            .spawn(udp_listener_task(node_id, stack, inbox_sender))
            .unwrap();

        spawner
            .spawn(udp_sender_task(node_id, stack, outbox_receiver))
            .unwrap();

        UdpTransport::new(node_id, outbox_sender, inbox_receiver)
    }

    /// Create network stack for a node and spawn its runner task
    ///
    /// # Safety
    /// Each node_id must be unique (1-5) to ensure distinct static resources
    /// are used without aliasing. This is guaranteed by the caller loop.
    unsafe fn create_network_stack_and_spawn_runner(
        node_id: u8,
        spawner: Spawner,
    ) -> embassy_net::Stack<'static> {
        let driver = MockNetDriver::new(node_id, &NETWORK_BUS);
        let config = get_node_config(node_id);
        let resources = config::get_node_resources(node_id);
        let seed = 0x0123_4567_89AB_CDEF_u64 + node_id as u64;

        let (stack, runner) = embassy_net::new(driver, config, &mut resources.resources, seed);

        spawner.spawn(net_stack_task(node_id, runner)).unwrap();

        stack
    }
}

impl Default for UdpTransportHub {
    fn default() -> Self {
        Self::new()
    }
}

#[embassy_executor::task(pool_size = 5)]
async fn net_stack_task(_node_id: u8, mut runner: embassy_net::Runner<'static, MockNetDriver>) {
    runner.run().await
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
