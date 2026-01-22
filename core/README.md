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

**Architecture:**
- ✅ Pure `no_std` implementation (only requires `alloc`)
- ✅ Trait-based abstraction for all external dependencies
- ✅ Zero `unsafe` code in core logic
- ✅ Event-driven design (no background threads)
- ✅ Compile-time polymorphism (no dynamic dispatch)
- ✅ Comprehensive observer pattern for telemetry

### 🔄 Validation Status

The core has been validated through:
- ✅ **Embassy**: 5-node cluster running on QEMU with UDP transport
- ✅ **Raft-Sim**: Deterministic test harness with adversarial network conditions
- ✅ **Client Request Handling**: Full write-path with commit acknowledgments
- ✅ **Leader Re-election**: Recovery from failures and partitions
- ✅ **Snapshot Creation & Transfer**: Automatic log compaction and follower catch-up
- ✅ **Crash Recovery**: Node restart with state restoration from snapshots
- ✅ **110+ Tests Passing**: 33 test files covering all core features

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

The environment is responsible for:
1. Polling timers and converting expirations to `Event::TimerFired`
2. Receiving network messages and converting to `Event::Message`
3. Accepting client requests and converting to `Event::ClientCommand`
4. Draining the outbox and physically sending messages via the transport

### 3. **No Hidden State**

All Raft state is owned by `RaftNode`:
- No global variables
- No thread-local storage
- No ambient context
- 100% deterministic given the same event sequence

This enables:
- Reproducible bugs
- Deterministic testing
- Formal verification (future)
- Easy snapshotting of entire cluster state

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

Implementations can:
- Log to `defmt`, `log`, or structured JSON
- Emit Prometheus metrics
- Generate distributed traces (OpenTelemetry)
- Capture events for test assertions

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
- [components/message_handler.rs](src/components/message_handler.rs) - **All Raft message processing** (~830 LOC)

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

### Test Organization

The test suite is organized by feature/scenario:
```
sim/tests/ (33 test files)
├── Component Tests (5 files)
│   ├── election_manager_tests.rs
│   ├── log_replication_manager_tests.rs
│   ├── snapshot_manager_tests.rs
│   ├── config_change_manager_tests.rs
│   └── role_transition_manager_tests.rs
├── MessageHandler Tests (1 file)
│   └── message_handler_tests.rs (7 tests)
├── Election Protocol Tests (6 files)
│   ├── basic_leader_election_tests.rs
│   ├── pre_vote_tests.rs
│   ├── split_leader_election_tests.rs
│   ├── timed_election_tests.rs
│   └── ...
├── Replication & Safety Tests (9 files)
│   ├── client_payload_replication_tests.rs
│   ├── commit_index_advancement_tests.rs
│   ├── log_matching_property_tests.rs
│   └── ...
├── Snapshot & Compaction Tests (5 files)
│   ├── snapshot_creation_protocol_tests.rs
│   ├── snapshot_infrastructure_tests.rs
│   ├── install_snapshot_candidate_tests.rs
│   └── crash_recovery_with_snapshots_tests.rs
└── Configuration Tests (2 files)
    ├── config_api_tests.rs
    └── single_node_cluster_tests.rs
```

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

**Test Coverage (110+ tests across 33 test files):**
- ✅ Snapshot creation after threshold exceeded
- ✅ Snapshot metadata tracking and persistence
- ✅ InstallSnapshot RPC with chunked transfer
- ✅ Log compaction and entry discard
- ✅ Follower catch-up via snapshot transfer
- ✅ State machine snapshot/restore round-trip
- ✅ Node restart with snapshot restoration
- ✅ Multiple restart cycles with intermediate snapshots
- ✅ Follower crash during snapshot transfer
- ✅ Recovery without snapshot (uncommitted entries discarded)
- ✅ Continued operation after recovery
- ✅ MessageHandler isolation tests (7 tests)

**Operational Value**: ✅ Production-ready bounded memory for long-running clusters

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

**Status**: ⚠️ **Moderate Readiness (40%)**

**Currently Available:**
- ✅ Leader tracking via observer/global state
- ✅ Commit index is tracked and updated
- ✅ State machine has `get()` method for queries

**Missing Components:**
- ❌ No lease-based reads (leader must confirm leadership)
- ❌ No heartbeat acknowledgment tracking (needed for lease mechanism)
- ❌ No read index tracking (leader must record commit index at read time)
- ❌ No read request queuing (reads should wait for commit advancement)
- ❌ `ClientRequest` in embassy is write-only

**Required Changes:**

1. **Add Lease Mechanism:**
   ```rust
   struct LeaderLease {
       valid_until: Instant,
       last_quorum_contact: Instant,
   }
   ```

2. **Track Heartbeat Acknowledgments:**
   - Record timestamp of last successful heartbeat to each follower
   - Lease is valid if majority responded within heartbeat interval

3. **Add Read-Index Protocol:**
   - Leader records `read_index = commit_index` when read arrives
   - Leader sends heartbeat to confirm leadership
   - Leader waits for `commit_index >= read_index`
   - Then serve read from state machine

4. **Extend Client API:**
   ```rust
   pub enum ClientRequest {
       Write { payload: String, response_tx: ... },
       Read { key: String, response_tx: ... },
   }
   ```

5. **Add Observer Events:**
   ```rust
   fn read_served(&mut self, node: NodeId, key: &str, commit_index: LogIndex);
   fn lease_expired(&mut self, node: NodeId);
   ```

**Implementation Effort**: Medium (1-2 weeks)
**Operational Value**: Medium (reduces load on leader, faster reads)
**Risk**: Low (doesn't affect write path)

---

## Recommended Implementation Order

### Phase 1: Log Compaction + Snapshots ✅
**Status**: ✅ **COMPLETE**
110+ tests passing across 33 test files

**Completed Features:**
- ✅ Automatic snapshot creation at configurable threshold
- ✅ InstallSnapshot RPC with chunked transfer
- ✅ Snapshot storage interface and persistence
- ✅ State machine snapshot/restore API
- ✅ Log compaction (discard entries before snapshot)
- ✅ Crash recovery with snapshot restoration
- ✅ Follower catch-up via snapshot transfer

**Test Coverage:**
1. ✅ **Snapshot Creation**: Leader creates snapshot after log exceeds threshold
2. ✅ **Snapshot Transfer**: InstallSnapshot RPC with chunked data transfer
3. ✅ **Log Compaction**: Leader discards compacted entries
4. ✅ **Follower Catch-Up**: Lagging followers receive snapshots
5. ✅ **Crash Recovery**: Node restart with snapshot restoration
6. ✅ **Multiple Restarts**: Snapshot persistence across restart cycles
7. ✅ **Crash During Transfer**: Follower recovery from partial snapshot
8. ✅ **Recovery Without Snapshot**: Uncommitted entries correctly discarded
9. ✅ **Continued Operation**: Recovered nodes rejoin consensus

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

**Required New Tests:**
1. **Add Server Test**: Leader receives `AddServer` request, commits configuration change (C_old → C_old,new → C_new), new node catches up and participates in consensus
2. **Remove Server Test**: Leader removes node via joint consensus, cluster continues operating with reduced membership
3. **Joint Consensus Commit Test**: During transition, verify that entries require majority from both C_old and C_old,new configurations
4. **Configuration Log Entry Test**: Configuration changes stored as special log entries, replicated and applied deterministically
5. **Leader Removal Test**: Leader removes itself from cluster, steps down after committing C_new, new leader elected from remaining nodes
6. **Concurrent Reconfig Prevention Test**: Second configuration change request rejected while first is in progress (C_old,new active)
7. **Split-Brain Prevention Test**: Partitioned old configuration cannot commit without new members (joint majority required)
8. **New Member Catch-Up Test**: Added node receives full log (or snapshot), transitions from non-voting to voting member after catching up
9. **Self-Removal Edge Case Test**: Node removes itself from cluster, stops participating after C_new committed
10. **Multiple Sequential Reconfigs Test**: Cluster successfully processes multiple add/remove operations in sequence (one at a time)

---

### Phase 4: Linearizable Reads
**Priority**: Medium
**Status**: Planning phase

**Rationale:**
- Good user-facing feature
- Validates observer extensibility
- Relatively isolated from core write path

**Success Criteria:**
- Clients can issue read-only queries
- Reads are linearizable (no stale data)
- Leader lease mechanism prevents split-brain reads

**Required New Tests:**
1. **Leader Lease Validation Test**: Leader establishes lease via heartbeat quorum, serves reads during valid lease period
2. **Lease Expiration Test**: Leader stops serving reads after lease expires (no heartbeat quorum), must re-establish lease
3. **Read-Index Protocol Test**: Leader records commit_index at read time, waits for advancement, then serves read from state machine
4. **Stale Read Prevention Test**: Partitioned leader cannot serve reads (lease expires without quorum), new leader elected elsewhere
5. **Read Queuing Test**: Multiple concurrent reads wait for commit_index advancement, all served in order after confirmation
6. **Linearizability Verification Test**: Read reflects all writes committed before read request (happens-before ordering preserved)
7. **Follower Read Rejection Test**: Followers reject read requests or forward to leader (no stale reads from non-leaders)

---

## Future Considerations

### Beyond Raft Paper
- **Multi-Raft**: Multiple Raft groups sharing resources
- **Leadership Transfer**: Graceful handoff for maintenance
- **Batch Writes**: Amortize consensus overhead
- **Parallel Log Replication**: Pipeline entries to followers

### Formal Verification
- **TLA+ Specification**: Model-check core state transitions
- **Proof of Correctness**: Mechanized proofs in Coq/Lean
- **Fuzzing**: Property-based testing with `proptest`

### Optimizations
- **Zero-copy Serialization**: Avoid allocations in hot path
- **SIMD for CRC**: Faster integrity checks
- **Batch AppendEntries**: Reduce RPC overhead

---

## Contributing

When implementing advanced features:

1. **Keep core pure**: No environment dependencies
2. **Extend traits**: Don't break existing implementations
3. **Add observer events**: Make new behavior observable
4. **Write tests first**: Validate in deterministic harness
5. **Update documentation**: Explain trade-offs and design decisions

---

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
