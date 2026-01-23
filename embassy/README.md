# Embassy: Raft Consensus in `no_std` Embedded Environment

## Overview

**Embassy** is a proof-of-concept demonstration that a full Raft consensus cluster can run in a `no_std` embedded environment with **real UDP-based networking**. Built on the [Embassy](https://embassy.dev) async runtime for Cortex-M microcontrollers, this project simulates a 5-node Raft cluster running on QEMU with simulated Ethernet.

### Key Achievement

✅ **Running a distributed consensus protocol (Raft) entirely in `no_std`**
- No standard library (no heap allocations via `std`, only `alloc`)
- Real network stack (embassy-net with UDP)
- Multiple async tasks coordinated via Embassy executor
- Serialization with postcard (no protobuf/JSON dependencies)
- Simulated Ethernet driver for testing without hardware
- Full Raft feature set including snapshots and dynamic membership

## Architecture

### Core Components

```
embassy/
├── src/
│   ├── main.rs                       # Entry point, spawns cluster
│   ├── embassy_node.rs               # Main Raft node loop (generic over T, S)
│   ├── configurations/               # Feature-gated configurations
│   │   ├── mod.rs                    # Single location for all cfg logic
│   │   ├── memory_channel/           # In-memory storage + channel transport
│   │   │   ├── mod.rs
│   │   │   ├── setup.rs              # Cluster initialization
│   │   │   ├── storage.rs            # In-memory storage implementation
│   │   │   └── transport.rs          # Channel-based transport
│   │   └── semihosting_udp/          # Persistent storage + UDP transport
│   │       ├── mod.rs
│   │       ├── setup.rs              # Cluster initialization
│   │       └── storage/              # Semihosting-based storage
│   │           ├── mod.rs
│   │           └── storage.rs        # File I/O via syscalls
│   ├── embassy_storage.rs            # Storage trait definition
│   ├── embassy_timer.rs              # Election/heartbeat timers
│   ├── transport/                    # Transport implementations
│   │   ├── async_transport.rs        # Transport trait
│   │   ├── embassy_transport.rs      # Raft-to-async bridge
│   │   ├── channel_transport.rs      # In-memory channels
│   │   └── udp/                      # UDP network transport
│   │       ├── transport.rs
│   │       ├── driver.rs
│   │       ├── serde.rs
│   │       └── config.rs
│   └── [other adapters]
├── persistency/                      # Persistent storage files (gitignored)
└── Cargo.toml
```

### Plug-in Architecture Pattern

The project implements a **compile-time dependency injection** system with four configuration modes:

1. **In-Memory + Channel** (`--features in-memory-storage,channel-transport`): Fast simulation, no persistence
2. **In-Memory + UDP** (`--features in-memory-storage,udp-transport`): Network simulation, no persistence
3. **Semihosting + Channel** (`--features semihosting-storage,channel-transport`): Persistent storage, fast transport
4. **Semihosting + UDP** (default: `--features semihosting-storage,udp-transport`): Full persistence + networking

The `configurations/mod.rs` module contains **all conditional compilation logic** (the only file with `#[cfg(...)]` attributes). It re-exports the appropriate `setup` module based on active features:

```rust
// main.rs is completely configuration-agnostic - zero cfg attributes!
configurations::setup::initialize_cluster(spawner, cancel, observer_level).await;
```

**Benefits:**
- Application code completely decoupled from configuration selection
- Clean plug-in pattern: swap storage + transport via Cargo features
- Zero runtime cost via monomorphization
- Features are mutually exclusive (enforced at compile-time)

## Running the Simulation

### Prerequisites

- Rust nightly toolchain
- `thumbv7em-none-eabihf` target installed:
  ```bash
  rustup target add thumbv7em-none-eabihf
  ```
- QEMU ARM system emulation:
  ```bash
  # Windows (via Chocolatey)
  choco install qemu

  # Linux
  sudo apt install qemu-system-arm
  ```

### Quick Start

**UDP Transport + Persistent Storage (Default):**
```powershell
# From embassy directory
cargo run --release --features semihosting-storage,udp-transport

# Or use the script
.\scripts\run_udp.ps1
```

**Channel Transport + In-Memory Storage:**
```powershell
cargo run --release --features in-memory-storage,channel-transport --no-default-features

# Or use the script
.\scripts\run_channel.ps1
```

### Persistent Storage

When using `--features semihosting-storage`, Raft state is persisted to files via QEMU semihosting:

**Storage Location:** `embassy/persistency/`

**Files Created:**
- `raft_node_{1-5}_metadata.bin` - Current term, voted_for (17 bytes each)
- `raft_node_{1-5}_log.bin` - Serialized log entries
- `raft_node_{1-5}_snapshot_metadata.bin` - Snapshot metadata
- `raft_node_{1-5}_snapshot_data.bin` - Snapshot data

**Implementation:** Uses `cortex-m-semihosting` syscalls (OPEN, WRITE, READ, FLEN, CLOSE) for file I/O through QEMU's semihosting interface.

**Persistence Guarantees:**
- Metadata written on term change or vote
- Log entries written on append
- Snapshots written on creation
- Files survive cluster restarts (crash recovery)

### Expected Output

```
INFO Starting 5-node Raft cluster in Embassy
INFO Using UDP transport (simulated Ethernet)
INFO WireRaftMsg serialization layer: COMPLETE ✓
INFO Network stacks created, waiting for configuration...
INFO Node 1 network configured: Some(StaticConfigV4 { ... })
...
INFO All UDP nodes started!
INFO Node 3 starting election (term 1)
INFO Node 3 became Leader for term 1
INFO Node 3 sending heartbeats...
```

## Technical Details

### Memory Constraints

- **Heap Size**: 128 KB (configurable in `heap.rs`)
- **Max Log Entries**: 256 per node (heapless::Vec)
- **Channel Sizes**:
  - Channel transport: 32 messages
  - UDP transport: 16 messages per node
- **UDP Buffer**: 4 KB per socket

### Network Configuration

Each node gets a static IP in the `10.0.0.0/24` subnet:
- Node 1: `10.0.0.1:9001`
- Node 2: `10.0.0.2:9002`
- Node 3: `10.0.0.3:9003`
- Node 4: `10.0.0.4:9004`
- Node 5: `10.0.0.5:9005`

### Serialization

Raft messages are serialized using **postcard** (a compact binary format):
- `RaftMsg` → `WireRaftMsg` (custom mirror type with Serde derives)
- No dynamic allocations during serialization
- Efficient for embedded environments (~100-500 bytes per message)

### Concurrency Model

Embassy's async runtime manages:
- 5 Raft node tasks
- 5 Network stack runners (UDP only)
- 5 UDP listener tasks (UDP only)
- 5 UDP sender tasks (UDP only)

All coordinated via `embassy-executor` with bounded channels for backpressure.

## Development

### Building for Different Configurations

```bash
# Check all configuration combinations
cargo check --target thumbv7em-none-eabihf --features semihosting-storage,udp-transport
cargo check --target thumbv7em-none-eabihf --features in-memory-storage,channel-transport --no-default-features
cargo check --target thumbv7em-none-eabihf --features semihosting-storage,channel-transport --no-default-features
cargo check --target thumbv7em-none-eabihf --features in-memory-storage,udp-transport --no-default-features

# Build release binary (default: semihosting-storage + udp-transport)
cargo build --release --target thumbv7em-none-eabihf

# Run tests (host environment)
cargo test --lib
```

### Code Quality

The codebase follows embedded best practices:
- ✅ Zero `unsafe` outside documented initialization
- ✅ No panics in critical paths
- ✅ Bounded memory usage
- ✅ Feature-gated optional dependencies
- ✅ Compile-time mutual exclusivity enforcement
- ✅ All conditional compilation isolated to `configurations/mod.rs`

### Limitations

This is a **proof-of-concept**, not production code:
- Fixed 5-node cluster (hardcoded)
- No log compaction/snapshotting yet (core supports it, embassy doesn't implement)
- No dynamic cluster membership (single-server changes implemented in core)
- QEMU-only (not tested on real hardware)

### Validated Features

- ✅ **Leader Election**: Randomized timeouts break split-vote deadlocks
- ✅ **Log Replication**: Entries replicated to quorum with consistency checks
- ✅ **Commit Index Advancement**: Leader tracks follower progress and advances commit
- ✅ **Client Request Routing**: Followers transparently forward to leader
- ✅ **Wait-for-Commit**: Clients block until replication completes
- ✅ **State Machine Application**: Committed entries applied in order
- ✅ **Persistent Storage**: Optional file-backed storage via semihosting
- ✅ **Graceful Shutdown**: Broadcast cancellation to all nodes

## Project Goals

### ✅ Achieved

1. **Prove Raft can run in `no_std`**: Demonstrated with a working 5-node cluster
2. **Real UDP networking**: Full TCP/IP stack via embassy-net (not just in-memory channels)
3. **Clean architecture**: Storage and transport abstractions with plug-in pattern
4. **Embassy integration**: Proper use of async tasks, timers, and channels
5. **Modular configuration**: Compile-time dependency injection via feature flags
6. **Leader election**: Randomized timeouts ensure fast convergence (typically 3-5 terms)
7. **Log replication**: Commands replicated to majority before acknowledgment
8. **Client request handling**: Transparent forwarding from followers to leader
9. **Commit-based acknowledgments**: Clients notified only after quorum replication
10. **Persistent storage**: File-backed Raft state via semihosting (optional)
11. **Graceful shutdown**: Broadcast cancellation mechanism for all nodes
12. **Zero-cost abstractions**: Monomorphization ensures no runtime overhead

### Future Enhancements (Out of Scope)

- [ ] Run on real hardware (STM32, nRF52, etc.)
- [ ] Flash driver integration (replace semihosting)
- [ ] Embassy implementation of snapshotting (core supports it)
- [ ] Embassy implementation of dynamic membership (core supports single-server changes)
- [ ] Network partition simulation

## Related Projects

- **raft-core**: The platform-agnostic Raft library this project uses
- **Embassy**: The async runtime powering the executor
- **embassy-net**: The embedded TCP/IP stack (based on smoltcp)

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0

## Acknowledgments

Built with:
- [Embassy](https://github.com/embassy-rs/embassy) - Async runtime for embedded Rust
- [smoltcp](https://github.com/smoltcp-rs/smoltcp) - TCP/IP stack
- [postcard](https://github.com/jamesmunns/postcard) - Compact serialization format
