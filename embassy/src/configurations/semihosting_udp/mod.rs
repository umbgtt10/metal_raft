// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Semihosting UDP Configuration
//!
//! Combines:
//! - UDP transport (network-based communication)
//! - Semihosting storage (persistent file I/O via QEMU syscalls)
//!
//! Ideal for testing networked Raft with persistent state in QEMU.

pub mod setup;
