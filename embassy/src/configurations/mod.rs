// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// Core component modules (always available)
pub mod storage;
pub mod transport;

#[cfg(not(any(
    all(feature = "in-memory-storage", feature = "channel-transport"),
    all(feature = "semihosting-storage", feature = "udp-transport")
)))]
compile_error!("Must select exactly one configuration: either 'in-memory-storage,channel-transport' or 'semihosting-storage,udp-transport'");

#[cfg(all(feature = "in-memory-storage", feature = "channel-transport"))]
pub mod memory_channel;

#[cfg(all(feature = "semihosting-storage", feature = "udp-transport"))]
pub mod semihosting_udp;

#[cfg(all(feature = "in-memory-storage", feature = "channel-transport"))]
pub use memory_channel::setup;

#[cfg(all(feature = "semihosting-storage", feature = "udp-transport"))]
pub use semihosting_udp::setup;
