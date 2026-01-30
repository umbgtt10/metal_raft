# Raft-Core: Platform-Agnostic Consensus Algorithm

## Overview

**Raft-Core** is a pure, `no_std`-compatible implementation of the Raft consensus algorithm. It is designed to be completely independent of any runtime, networking stack, or storage backend. The core provides the algorithmic "brain" of Raft while delegating all environment-specific concerns to pluggable trait implementations.

This design philosophy enables the same consensus logic to run unchanged across:
- Embedded microcontrollers (Embassy, RTOS)
- Standard Rust applications (Tokio, async-std)
- Deterministic test harnesses
- Cloud-native deployments

---

## Features

### Implemented Features ✅

#### 1. Core Raft Protocol ✅

**Leader Election:**
- Randomized election timeouts
- Vote request/response handling
- Candidate to Leader transition
- Split vote resolution

**Log Replication:**
- AppendEntries RPC with consistency checks
- Follower log matching and repair
- Commit index advancement (quorum-based)
- State machine application (in-order, idempotent)

**Safety Guarantees:**
- Election Safety: At most one leader per term
- Leader Append-Only: Leaders never overwrite entries
- Log Matching: Identical entries at same index across nodes
- Leader Completeness: Committed entries present in all future leaders
- State Machine Safety: Deterministic, in-order application

**Pre-Vote Protocol:**
- Prevents disruptions from partitioned nodes
- Term inflation protection
- No safety impact, pure liveness improvement

#### 2. Log Compaction & Snapshots ✅

- Automatic snapshot creation at configurable threshold
- InstallSnapshot RPC with chunked transfer
- Snapshot metadata tracking (last_included_index/term)
- State machine snapshot/restore API
- Storage interface for snapshot persistence
- Log compaction (discard entries before snapshot)
- Crash recovery with snapshot restoration
- Follower catch-up via snapshot transfer

**Operational Value:** Production-ready bounded memory for long-running clusters

#### 3. Dynamic Membership ✅

**Single-Server Configuration Changes:**
- Add/remove one server at a time safely
- Configuration tracking and validation
- Catching-up servers (non-voting until synchronized)
- Configuration survives snapshots and crashes

**Status:** 70% foundation complete. Joint Consensus (multi-server changes) not implemented

#### 4. Lease-Based Linearizable Reads ✅

- Leader lease mechanism for high-performance reads
- Lease granted on commit advancement (quorum acknowledgment)
- Lease revoked on step down (leadership loss)
- Safety guarantee: lease_duration < election_timeout

### Missing Features

#### Joint Consensus (Multi-Server Configuration Changes)
#### Leadership Transfer
#### Read Index Protocol

### Validation

See [../validation/README.md](../validation/README.md) for:
- Test infrastructure and philosophy
- Detailed test coverage by feature
- Running instructions

## Architecture Principles

### 1. **Radical Abstraction**

The core knows nothing about:
- Async runtimes (Tokio, Embassy, std::thread)
- Networking protocols (TCP, UDP, gRPC, in-memory channels)
- Storage backends (memory, flash, disk, cloud)
- Serialization formats (JSON, bincode, postcard, protobuf)

All interactions happen through traits:

```rust
pub trait Transport {
    type Payload: Clone;
    type LogEntries: LogEntryCollection<Payload = Self::Payload>;

    fn send(&mut self, target: NodeId, msg: RaftMsg<Self::Payload, Self::LogEntries>);
}

pub trait Storage {
    type Payload: Clone;
    type LogEntryCollection: LogEntryCollection<Payload = Self::Payload>;

    fn current_term(&self) -> Term;
    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>>;
    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]);
    // ...
}

pub trait StateMachine {
    type Payload;

    fn apply(&mut self, payload: &Self::Payload);
}
```

### 2. **Event-Driven Design**

The core operates as a pure state machine:

```rust
pub fn on_event(&mut self, event: Event<P, L>) {
    match event {
        Event::TimerFired(kind) => self.handle_timer(kind),
        Event::Message { from, msg } => self.handle_message(from, msg),
        Event::ClientCommand(payload) => self.handle_client_command(payload),
    }
}
```

### 3. **No Hidden State**

All Raft state is owned by `RaftNode`:
- No global variables
- No thread-local storage
- No ambient context
- 100% deterministic given the same event sequence

### 4. **Zero-Cost Abstractions**

All abstractions resolve at compile-time:
- Generic trait bounds (no `dyn Trait`)
- Fixed-size collections (via `heapless` or similar)
- Inline-friendly design
- No allocations in hot paths (except appending log entries)

### 5. **Observable by Design**

The `Observer` trait provides structured visibility:
- **Essential**: Leader changes, critical state transitions
- **Info**: Elections, commits, important operations
- **Debug**: Votes, heartbeats, detailed operations
- **Trace**: Every message, every timer

---

## Benefits of This Architecture

### For Embedded Systems
- **Minimal footprint**: Core is <20KB compiled
- **No heap fragmentation**: Fixed-size collections
- **Deterministic**: No hidden async tasks or background threads
- **Portable**: Works on Cortex-M, RISC-V, Xtensa, etc.

### For Testing
- **Deterministic replay**: Same events → same outcomes
- **Time control**: Inject timer events on demand
- **Network simulation**: Full control over message delivery
- **Chaos testing**: Inject partitions, delays, crashes

### For Production
- **Observability**: Rich telemetry without intrusive instrumentation
- **Debuggability**: No "heisenbugs" from race conditions
- **Pluggable backends**: Swap storage/network without core changes
- **Multi-environment**: Same logic in dev, staging, prod

### For Correctness
- **Testable**: Core has zero untestable dependencies
- **Reviewable**: ~2000 LOC of pure algorithm
- **Provable**: Formal methods can analyze state transitions
- **Auditable**: No hidden complexity in dependencies

---

## Code Organization

### Core Components Architecture

**State Management:**
- [node_state.rs](src/node_state.rs) - Follower/Candidate/Leader states
- [raft_node.rs](src/raft_node.rs) - Main Raft node implementation

**Component Modules:**
- [components/election_manager.rs](src/components/election_manager.rs) - Election timeouts, vote tracking
- [components/log_replication_manager.rs](src/components/log_replication_manager.rs) - Follower tracking, next_index, match_index
- [components/snapshot_manager.rs](src/components/snapshot_manager.rs) - Snapshot creation, threshold management
- [components/config_change_manager.rs](src/components/config_change_manager.rs) - Configuration change protocol
- [components/role_transition_manager.rs](src/components/role_transition_manager.rs) - State transitions, initialization
- [components/leader_lease.rs](src/components/leader_lease.rs) - Leader lease for linearizable reads
- [components/message_handler.rs](src/components/message_handler.rs) -

**Abstractions:**
- [transport.rs](src/transport.rs) - Network abstraction
- [storage.rs](src/storage.rs) - Persistence abstraction
- [state_machine.rs](src/state_machine.rs) - Application state abstraction
- [timer_service.rs](src/timer_service.rs) - Timer abstraction
- [observer.rs](src/observer.rs) - Telemetry/instrumentation abstraction

### MessageHandler: Central Processing Unit

The `MessageHandler` is the heart of Raft message processing, implementing:
- **Vote handling**: RequestVote, PreVote RPCs with election safety
- **Log replication**: AppendEntries with consistency checks and commit advancement
- **Snapshot transfer**: InstallSnapshot with chunked data transfer
- **Client commands**: Write operations submitted to leader
- **Config changes**: AddServer/RemoveServer protocol (joint consensus prep)
- **Timer handling**: Election and heartbeat timeout processing

**Current State**: ~340 lines with clear separation of concerns via helper methods
**Testability**: Isolated from RaftNode via MessageHandlerContext

---

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
