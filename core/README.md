# Raft-Core: Platform-Agnostic Consensus Algorithm

## Overview

**Raft-Core** is a pure, `no_std`-compatible implementation of the Raft consensus algorithm. It is designed to be completely independent of any runtime, networking stack, or storage backend. The core provides the algorithmic "brain" of Raft while delegating all environment-specific concerns to pluggable trait implementations.

This design philosophy enables the same consensus logic to run unchanged across:
- Embedded microcontrollers (Embassy, RTOS)
- Standard Rust applications (Tokio, async-std)
- Deterministic test harnesses
- Cloud-native deployments

---

## Current Status

### ✅ Implemented Features

**Core Raft Protocol:**
- ✅ **Leader Election**
  - Randomized election timeouts
  - Vote request/response handling
  - Candidate to Leader transition
  - Split vote resolution

- ✅ **Log Replication**
  - AppendEntries RPC with consistency checks
  - Follower log matching and repair
  - Commit index advancement (quorum-based)
  - State machine application (in-order, idempotent)

- ✅ **Safety Guarantees**
  - Election Safety: At most one leader per term
  - Leader Append-Only: Leaders never overwrite entries
  - Log Matching: Identical entries at same index across nodes
  - Leader Completeness: Committed entries present in all future leaders
  - State Machine Safety: Deterministic, in-order application

- ✅ **Log Compaction & Snapshots**
  - Automatic snapshot creation at configurable threshold
  - InstallSnapshot RPC with chunked transfer
  - Snapshot metadata tracking (last_included_index/term)
  - State machine snapshot/restore API
  - Storage interface for snapshot persistence
  - Log compaction (discard entries before snapshot)
  - Crash recovery with snapshot restoration
  - Follower catch-up via snapshot transfer

- ✅ **Pre-Vote Protocol**
  - Prevents disruptions from partitioned nodes
  - Term inflation protection
  - No safety impact, pure liveness improvement

- ✅ **Dynamic Membership (Single-Server Changes)**
  - Add/remove one server at a time safely
  - Configuration tracking and validation
  - Catching-up servers (non-voting until synchronized)
  - Configuration survives snapshots and crashes

- ✅ **Lease-Based Linearizable Reads**
  - Leader lease mechanism for high-performance reads
  - Lease granted on commit advancement (quorum acknowledgment)
  - Lease revoked on step down (leadership loss)
  - Safety guarantee: lease_duration < election_timeout

### Validation

All features are comprehensively tested with 232 tests (156 core unit + 76 validation integration) covering safety properties, adversarial scenarios, and multi-environment execution.

See [../validation/README.md](../validation/README.md) for:
- Test infrastructure and philosophy
- Detailed test coverage by feature
- Running instructions

---

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

The Raft core is organized into focused, testable components:

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

**Current State**: Single 830-line file with clear separation of concerns via helper methods
**Testability**: Isolated from RaftNode via MessageHandlerContext (7 dedicated tests in message_handler_tests.rs)



---

## Advanced Features: Readiness Assessment

The current implementation covers the core Raft protocol. The following advanced features are described in the Raft paper and required for production readiness. Here is a detailed analysis of implementation readiness:

### 1. Log Compaction & Snapshots

**Status**: ✅ **COMPLETE**

**Implemented Components:**
- ✅ State Machine abstraction with `apply()` method
- ✅ Snapshot storage interface (`save_snapshot()`, `load_snapshot()`, `get_snapshot()`)
- ✅ `InstallSnapshot` RPC in `RaftMsg` enum with chunked transfer
- ✅ Snapshot metadata tracking (last_included_index/term)
- ✅ StateMachine snapshot API (`create_snapshot()`, `restore_from_snapshot()`)
- ✅ Storage trait `discard_entries_before()` for log compaction
- ✅ Configurable threshold for triggering compaction (default: 10 entries)
- ✅ Automatic snapshot creation at threshold
- ✅ Snapshot transfer to lagging followers
- ✅ **Crash recovery** - nodes restore state from snapshots on restart
- ✅ Observer pattern extended for snapshot events

**Implementation Details:**

1. **Storage Trait Extensions:**
   ```rust
   fn save_snapshot(&mut self, snapshot: Snapshot<Self::SnapshotData>);
   fn load_snapshot(&self) -> Option<Snapshot<Self::SnapshotData>>;
   fn get_snapshot(&self) -> Option<Snapshot<Self::SnapshotData>>;
   fn discard_entries_before(&mut self, index: LogIndex);
   fn get_snapshot_chunk(&self, offset: usize, max_size: usize) -> Option<Vec<u8>>;
   ```

2. **StateMachine Trait Extensions:**
   ```rust
   fn create_snapshot(&self) -> Self::SnapshotData;
   fn restore_from_snapshot(&mut self, data: &Self::SnapshotData);
   ```

3. **InstallSnapshot RPC:**
   ```rust
   RaftMsg::InstallSnapshot {
       term: Term,
       leader_id: NodeId,
       last_included_index: LogIndex,
       last_included_term: Term,
       offset: usize,
       data: Vec<u8>,
       done: bool,
   }
   ```

4. **Compaction Trigger Logic:**
   - Automatic snapshot creation when log exceeds threshold
   - Leader detects followers needing snapshots (next_index <= last_included_index)
   - Chunked snapshot transfer for large state machines
   - Log compaction after successful snapshot creation

5. **Crash Recovery:**
   - On node restart, load latest snapshot from storage
   - Restore state machine to snapshot state
   - Update last_applied to snapshot's last_included_index
   - **Never replay uncommitted log entries** (Raft safety requirement)

**Operational Value**: Production-ready bounded memory for long-running clusters

---

### 2. Dynamic Membership (Joint Consensus)

**Status**: ⚠️ **Partially Implemented (70%)**

**Currently Implemented:**
- ✅ `ConfigurationChange` enum (AddServer/RemoveServer)
- ✅ `Configuration` struct with quorum calculation
- ✅ `ConfigChangeManager` for validation and tracking
- ✅ Integration in `RaftNode` and `MessageHandler`
- ✅ `EntryType::ConfigChange` in log entries
- ✅ Observer events for configuration changes
- ✅ Single-Server Changes infrastructure (Week 1 tasks from implementation plan)

**Missing for Full Joint Consensus:**
- ❌ Two-configuration state (C_old and C_old,new simultaneous tracking)
- ❌ Joint quorum calculation (majority from both configs)
- ❌ Automatic transition from C_old,new to C_new
- ❌ Tests for joint consensus scenarios

**Implementation Effort**: Medium (2-3 weeks for Joint Consensus upgrade)
**Operational Value**: High (enables cluster scaling without downtime)
**Risk**: High (affects core election/commit logic)

---

### 3. Linearizable Reads (Read-Only Queries)

**Status**: ✅ **COMPLETE**

**Implemented Components:**
- ✅ `LeaderLease` struct with grant/revoke/is_valid methods
- ✅ Clock abstraction (CLK parameter) for time-based operations
- ✅ Lease granted on commit advancement (quorum acknowledgment)
- ✅ Lease revoked on step down (leadership loss)
- ✅ Safety guarantee: `lease_duration < election_timeout`
- ✅ Integrated into `RaftNode` and `MessageHandler`
- ✅ Observer events for lease operations

**Implementation Details:**

1. **LeaderLease Structure:**
   ```rust
   pub struct LeaderLease<CLK: Clock> {
       expiration: Option<CLK::Instant>,
       lease_duration: Duration,
   }
   ```

2. **Lease Grant Logic:**
   - Triggered on commit index advancement (quorum acknowledgment)
   - Located in `handle_append_entries_response()`
   - Ensures leader has recent quorum contact before serving reads

3. **Lease Revoke Logic:**
   - Triggered on step down (leadership loss)
   - Located in `step_down()` common handler
   - Prevents stale reads from former leaders

4. **Safety Invariant:**
   - `lease_duration < election_timeout` enforced at construction
   - Prevents reads during leadership transitions
   - Leader cannot grant stale lease after losing leadership

5. **Observer Events:**
   ```rust
   fn lease_granted(&mut self, node: NodeId, expiration: CLK::Instant);
   fn lease_revoked(&mut self, node: NodeId);
   ```

**Operational Value**: High (50-100x read performance improvement)

---

## Recommended Implementation Order

### Phase 1: Log Compaction + Snapshots ✅
**Status**: ✅ **COMPLETE**

**Completed Features:**
- ✅ Automatic snapshot creation at configurable threshold
- ✅ InstallSnapshot RPC with chunked transfer
- ✅ Snapshot storage interface and persistence
- ✅ State machine snapshot/restore API
- ✅ Log compaction (discard entries before snapshot)
- ✅ Crash recovery with snapshot restoration
- ✅ Follower catch-up via snapshot transfer

**Operational Impact**: Clusters can now run indefinitely with bounded memory usage

---

### Phase 2: Pre-Vote Protocol ✅
**Status**: ✅ **COMPLETE**

**Implementation**: Fully integrated with 6 dedicated tests
**Problem Solved**: Prevents disruptive elections from partitioned nodes
**Impact**: Stable leaders not disrupted by isolated nodes with inflated terms

---

### Phase 3: Dynamic Membership - Joint Consensus Upgrade
**Priority**: High
**Status**: 70% foundation complete, needs Joint Consensus logic

**Rationale:**
- Foundation already built (Single-Server Changes)
- Critical for production operational flexibility
- Most complex remaining feature

**Success Criteria:**
- Cluster can add/remove nodes without downtime
- Joint consensus prevents split-brain during reconfig
- Edge cases handled (leader removal, self-removal, etc.)

---

### Phase 4: Linearizable Reads ✅
**Priority**: Medium
**Status**: ✅ **COMPLETE**

**Rationale:**
- High-value user-facing feature (50-100x read performance)
- Validates clock abstraction and observer extensibility
- Relatively isolated from core write path

**Success Criteria (All Met):**
- ✅ Leader lease mechanism with grant/revoke logic
- ✅ Reads are linearizable (no stale data)
- ✅ Lease safety: `lease_duration < election_timeout`

**Completed Work:**
- LeaderLease component with clock abstraction
- Integration in MessageHandler (grant on commit, revoke on step down)
- Observer events for lease operations
- Safety invariant enforcement at construction

---

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
