// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Configuration modules combining storage and transport implementations
//!
//! This module contains:
//! - Component implementations (storage, transport)
//! - Complete configurations that combine components for specific use cases

// Core component modules (always available)
pub mod storage;
pub mod transport;

// Ensure at least one configuration is selected
#[cfg(not(any(
    all(feature = "in-memory-storage", feature = "channel-transport"),
    all(feature = "semihosting-storage", feature = "udp-transport")
)))]
compile_error!("Must select exactly one configuration: either 'in-memory-storage,channel-transport' or 'semihosting-storage,udp-transport'");

// Configuration modules (feature-gated)
#[cfg(all(feature = "in-memory-storage", feature = "channel-transport"))]
pub mod memory_channel;

#[cfg(all(feature = "semihosting-storage", feature = "udp-transport"))]
pub mod semihosting_udp;

// Re-export the active configuration's setup module
#[cfg(all(feature = "in-memory-storage", feature = "channel-transport"))]
pub use memory_channel::setup;

#[cfg(all(feature = "semihosting-storage", feature = "udp-transport"))]
pub use semihosting_udp::setup;
