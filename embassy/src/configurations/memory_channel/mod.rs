// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Memory Channel Configuration
//!
//! Combines:
//! - Channel transport (in-memory message passing)
//! - In-memory storage (non-persistent)
//!
//! Ideal for testing and development in QEMU without network simulation.

pub mod setup;
