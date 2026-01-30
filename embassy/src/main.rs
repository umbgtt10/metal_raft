// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![no_std]
#![no_main]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::Duration;
use panic_semihosting as _;

#[macro_use]
pub mod logging;
pub mod cancellation_token;
pub mod client_channel_hub;
pub mod collections;
pub mod config;
pub mod configurations;
pub mod embassy_node;
pub mod embassy_observer;
pub mod embassy_state_machine;
pub mod embassy_storage;
pub mod embassy_timer;
pub mod heap;
pub mod led_state;
pub mod raft_client;
pub mod time_driver;

use cancellation_token::CancellationToken;
use client_channel_hub::ClientChannelHub;

use crate::configurations::setup::initialize_client;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    heap::init_heap();

    let mut p = cortex_m::Peripherals::take().unwrap();
    time_driver::init(&mut p.SYST);

    info!("Starting 5-node Raft cluster in Embassy");
    info!("Runtime: 30 seconds");

    let cancel = CancellationToken::default();

    let observer_level = config::get_observer_level();
    info!("Observer level: {:?}", observer_level);

    let client_channel_hub = ClientChannelHub::new();
    let client =
        initialize_client(&client_channel_hub, spawner, cancel.clone(), observer_level).await;

    info!("All nodes started.");

    info!("Waiting for leader election...");
    embassy_time::Timer::after(Duration::from_secs(5)).await;

    // Submit test commands
    info!("Submitting test commands...");
    for i in 1..=3 {
        let command = alloc::format!("key{}=value{}", i, i);
        info!("Submitting: {}", command);
        match client.submit_command(command).await {
            Ok(_) => info!("Command {} committed successfully!", i),
            Err(e) => info!("Command {} failed: {:?}", i, e),
        }
    }
    info!("All commands processed!");

    // Wait for replication to complete
    embassy_time::Timer::after(Duration::from_millis(100)).await;

    // Demonstrate efficient reads: Access committed data from state machine
    info!("Reading back committed values...");
    for i in 1..=3 {
        let key = alloc::format!("key{}", i);
        match client.read_value(&key).await {
            Ok(Some(value)) => {
                info!("READ {} = {}", key, value);
            }
            Ok(None) => {
                info!("READ {} = <not found>", key);
            }
            Err(e) => {
                info!("READ {} failed: {:?}", key, e);
            }
        }
    }
    info!("All reads completed!");

    // Run for additional time to observe replication
    embassy_time::Timer::after(Duration::from_secs(1)).await;

    info!("1 seconds elapsed. Initiating graceful shutdown...");
    client.shutdown();

    // Give tasks time to finish
    embassy_time::Timer::after(Duration::from_millis(500)).await;

    info!("Shutdown complete. Exiting.");
    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
}
