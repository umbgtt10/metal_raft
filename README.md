# MetalRaft — A Multi-Environment, Correctness-First Raft Implementation

## Overview

This project implements the **Raft consensus algorithm** once and realizes it across multiple execution environments — from a fully deterministic in-memory simulator, to `no_std + Embassy`, and finally to a production-grade AWS/Kubernetes deployment.

The initial inspiration and methodological foundation for this work comes from the **MIT distributed systems labs (6.824 / 6.5840)**, particularly the Raft exercise. The project builds on those ideas but deliberately extends them toward stronger abstraction boundaries, multi-environment realizations, and production-grade operability.

The core principle is simple and non-negotiable:

> **Correctness is proven in simulation. Infrastructure is added only after the logic is frozen.**

This repository is intentionally structured to separate **algorithmic correctness** from **operational concerns**.

---

## Design Philosophy

### 1. One Core, Many Realizations

* Raft logic is implemented **exactly once** in a technology-agnostic core.
* All environment-specific concerns (runtime, networking, storage, observability) are layered *around* the core.
* The Raft core does not know:

  * which async runtime it runs on
  * how messages are transported
  * how data is persisted
  * where or how it is deployed

### 2. Correctness Before Infrastructure

* The algorithm is validated using a **deterministic, adversarial test harness**.
* Network partitions, message drops, reordering, and crashes are simulated.
* Only after correctness is established do we introduce:

  * persistence
  * real networking
  * cloud infrastructure

### 3. Infrastructure Is a Plugin

* Tokio, gRPC, Kubernetes, AWS, Grafana, Jaeger, etc. are **realizations**, not dependencies.
* The Raft core remains close to "bare metal" and compatible with `no_std`.

### 4. Monomorphization-First Architecture

* This project serves as a **design exploration** for a larger future effort, demonstrating that complex distributed algorithms like Raft can be implemented entirely through **compile-time generics** (monomorphization) without dynamic dispatch (`dyn Trait`).
* **Objective**: Prove that zero-cost abstractions scale to real-world consensus algorithms, including advanced features (log compaction, snapshots, dynamic membership, linearizable reads).
* **Key Insight**: Raft's complexity remains manageable with generics (~11 type parameters). This approach yields:
  * Zero runtime overhead (no vtable lookups)
  * Aggressive compiler optimizations (inlining, dead code elimination)
  * Embedded-friendly footprint
* **The Trade-off**: Even though Rust's zero-cost abstractions guarantee high performance through monomorphization, **the price to pay is cognitive load**. As type parameters proliferate (11+ generics), developers face:
  * Complex type signatures that obscure intent
  * Increased mental overhead when reasoning about code
  * Longer compile times as instantiations multiply
  * Reduced IDE responsiveness and error message clarity
* **Design Principle**: This project intentionally pushes monomorphization to its reasonable limits for Raft, demonstrating that at a certain complexity threshold, the pure monomorphization approach must be **appropriately combined with dynamic dispatch** (`dyn Trait`). This combination is architecturally significant: **generic abstractions signal and embody hard invariants** (compile-time enforced contracts that cannot be violated), while **dynamic dispatch signals soft architectural boundaries** (runtime flexibility points where behavior can vary without breaking core guarantees).

---

## Project Structure

```
metal_raft/
  core/               # no_std Raft algorithm (frozen logic)
  validation/         # std-based deterministic simulator & test harness
  embassy/            # no_std + Embassy realization (embedded target)
  docs/               # Architecture Decision Records and implementation plans
```

**Note**: Production runtime (Tokio) and deployment infrastructure (AWS/Kubernetes) are planned for future phases.

### `metal_raft/core`

* `#![no_std]` (optionally `alloc`)
* Implements:

  * Leader election (with Pre-Vote Protocol)
  * Log replication
  * Log compaction & snapshots
  * Commit rules
  * State transitions
  * Crash recovery
* Exposes abstract traits for:

  * Transport
  * TimerService
  * Storage
  * StateMachine
  * Observer (for instrumentation)
  * Collection abstractions (NodeCollection, MapCollection, etc.)

This crate is **never allowed** to depend on:

* async runtimes
* networking stacks
* serialization frameworks
* operating system facilities

---

## Implementation Phases

### Phase 0 — Raft Core ✅

* Pure in-memory implementation
* Deterministic execution
* No IO, no randomness
* Focus: safety & liveness

Exit criteria (all met):

* ✅ Single-leader guarantee
* ✅ Log-matching property
* ✅ Monotonic commit index
* ✅ Correct recovery from partitions

**Status**: Complete. 144+ tests passing across 21 validation test files. Validated in Embassy with UDP transport.

### Phase 1 — Log Compaction & Crash Recovery ✅

* Log compaction via snapshots
* Snapshot creation and transfer
* Crash recovery with snapshots
* Bounded memory usage for long-running clusters

Exit criteria (all met):

* ✅ Automatic log compaction at threshold
* ✅ Snapshot transfer to lagging followers
* ✅ Correct recovery after crash with snapshot
* ✅ Memory bounded even with high write load

**Status**: Complete. Validated in simulation and Embassy.

---

## Raft Enhancements

### Pre-Vote Protocol ✅ **ENABLED**

This implementation includes the **Pre-Vote Protocol** as described in Section 9.6 of Diego Ongaro's Raft thesis. Pre-vote is a critical optimization that prevents disruptions from partitioned or restarting nodes.

#### Why Pre-Vote?

In standard Raft, when a node's election timer fires, it immediately:
1. Increments its term
2. Starts an election
3. Requests votes from peers

**Problem**: A node that's been partitioned (can't reach majority) will repeatedly time out and increment its term. When the partition heals, this node contacts the cluster with a very high term, causing the current leader to step down unnecessarily.

**Solution**: Pre-vote adds a preliminary phase:
1. Node first asks "would you vote for me?" (pre-vote request)
2. If it receives majority approval, *then* it increments term and starts a real election
3. If pre-vote fails, term stays unchanged (no disruption)

#### Benefits:
- ✅ **Prevents term inflation** from partitioned nodes
- ✅ **Reduces disruptions** during network issues
- ✅ **Maintains liveness** - legitimate elections still proceed
- ✅ **No safety impact** - all Raft guarantees preserved

#### Implementation Details:
- Pre-vote uses current term (no increment)
- Pre-vote doesn't modify `voted_for` or persistent state
- Same log up-to-date checks as regular votes
- Majority of pre-votes required to proceed to real election
- Transparent to rest of system (no API changes)

**Status**: Fully implemented and tested. 6 dedicated pre-vote tests. Works across all environments (validation, embassy).

---

### Phase 2 — Simulation & Proof ✅

* Deterministic cluster simulator (`raft-validation`)
* Two test modes:
  * **Timeless**: Fully deterministic, no wall-clock time, total message control
  * **Timefull**: Wall-clock based, randomized timeouts, realistic timing
* Simulated network with:
  * partitions
  * message drops
  * reordering
  * latency simulation
* Crash / restart modeling
* 110+ integration tests covering:
  * Basic leader election
  * Log replication and commit
  * Network partitions and healing
  * Pre-vote protocol
  * Snapshot creation and transfer
  * Crash recovery with snapshots
  * Conflict resolution
  * State machine safety

Purpose:

* Produce **evidence of correctness** via integration-level tests
* All tests pass deterministically in both modes

---

### Phase 3 — `no_std + Embassy` ✅

* Embedded-compatible realization (`raft-embassy`)
* Small, static clusters (3-5 nodes)
* Embassy executor and timers
* UDP transport over simulated network
* In-memory storage with fixed-capacity buffers
* Postcard serialization (no_std compatible)

Purpose:

* Validate abstraction boundaries
* Demonstrate near bare-metal execution
* Prove portability without logic changes

**Status**: Complete. Validated with UDP transport in Embassy runtime. Same Raft core logic runs unchanged in both validation and Embassy environments.

---

### Phase 4 — Raft Advanced Features (In Progress)

* **Dynamic membership changes** (85% complete)
  - ✅ Single-Server Changes fully implemented (add/remove one node at a time)
  - ✅ Configuration tracking and quorum calculation
  - ✅ Catching-up servers (non-voting until synced)
  - ✅ Configuration survives snapshots and crashes
  - ✅ 21 validation test files including comprehensive config API tests
  - 🔲 Joint Consensus for multi-server changes (planned, ~2-3 weeks)
  - See: [docs/DYNAMIC_MEMBERSHIP_IMPLEMENTATION_PLAN.md](docs/DYNAMIC_MEMBERSHIP_IMPLEMENTATION_PLAN.md)
* **Read-only query optimization** (planned)
* **Leadership transfer** (planned)

Purpose:

* Complete Raft implementation for production readiness
* Safe reconfiguration without downtime
* Performance optimizations for read-heavy workloads

**Note**: Log compaction and Pre-Vote Protocol already complete (see above).

---

### Phase 5 — Cloud-Native (AWS)

* `std` + Tokio runtime
* Real networking (e.g. gRPC/TCP)
* Persistent storage (EBS)
* Docker images
* Kubernetes (StatefulSets)
* Helm charts

Observability:

* Metrics (Prometheus / Grafana)
* Tracing (OpenTelemetry / Jaeger)
* Structured logging

Serialization:

* `serde`, `sonic`, or similar — **strictly outside the core**

---

## Testing Strategy

* All Raft correctness tests run unchanged across phases
* Infrastructure additions must not alter test behavior
* Failures must be:

  * reproducible
  * deterministic
  * explainable

The test harness is treated as a **formal contract**.

---

## TODO: Advanced Raft Features & Documentation

### Algorithmic Features (To Be Implemented)

- � **Dynamic Membership**: Adding/removing nodes from the cluster (85% complete)
  - ✅ Single-Server Changes: Can safely add/remove one node at a time
  - ✅ Configuration tracking, quorum calculation, validation, catching-up servers
  - 🔲 Joint Consensus: Multi-server changes (~2-3 weeks remaining work)
  - Implementation plan: [docs/DYNAMIC_MEMBERSHIP_IMPLEMENTATION_PLAN.md](docs/DYNAMIC_MEMBERSHIP_IMPLEMENTATION_PLAN.md)
- 🔲 **Read-Only Queries**: Linearizable reads without log entries (leader leases)
- 🔲 **Leadership Transfer**: Graceful handoff for maintenance

### Architectural Decision Records (To Be Documented)

#### High Priority
- 🔲 **ADR-R11: Storage Durability Guarantees** - Fsync policy, WAL vs direct writes, durability/throughput tradeoffs
- 🔲 **ADR-R13: Transport Abstraction Design** - Why async-agnostic, message delivery guarantees, timeout handling
- 🔲 **ADR-R14: Error Propagation Strategy** - Panic vs Result, storage failure handling, network retry policies
- 🔲 **ADR-R16: Configuration Management** - Static vs dynamic config, election timeout tuning, snapshot threshold policy
- 🔲 **ADR-R19: Security Boundary Definition** - Trusted network assumption, TLS/auth in runtime layer, no BFT

#### Medium Priority
- 🔲 **ADR-R9: Observer Pattern for Instrumentation** - Trait-based observers, observable events, separation of concerns
- 🔲 **ADR-R10: Zero-Cost Abstractions for Telemetry** - Zero-overhead observability in `no_std`, Prometheus integration
- 🔲 **ADR-R12: Serialization Strategy** - Wire format choice, backward compatibility, schema evolution
- 🔲 **ADR-R17: Multi-Environment Realization Strategy** - Why Embassy sim exists, path to production, adapter responsibilities

#### Lower Priority
- 🔲 **ADR-R15: Deterministic Testing Philosophy** - Imperative vs property-based tests, chaos testing, soak tests
- 🔲 **ADR-R18: Memory Bounds & Resource Limits** - Max log size, message size limits, connection limits
- 🔲 **ADR-R20: Client Request Semantics** - Submit returns index not result, NotLeader retries, linearizability guarantees

---

## Non-Goals

This project intentionally does **not** aim to:

* implement Byzantine fault tolerance
* maximize throughput
* compete with production systems like etcd
* support large dynamic clusters in embedded targets

The goal is **clarity, correctness, and architectural rigor**.

---

## Motivation

This project exists to demonstrate:

* deep understanding of distributed consensus
* disciplined abstraction design
* correctness-first engineering
* portability across radically different environments
* production-ready observability practices

---

## Status

### Phase Progress

- ✅ **Phase 0 — Raft Core**: Complete (Leader election, log replication, commit rules, Pre-Vote Protocol)
- ✅ **Phase 1 — Log Compaction**: Complete (snapshots, InstallSnapshot RPC, crash recovery)
- ✅ **Phase 2 — Simulation & Proof**: Complete (144+ tests passing across 21 test files)
  - Deterministic test harness (timeless mode)
  - Wall-clock test harness (timefull mode)
  - Comprehensive coverage: elections, replication, snapshots, partitions, recovery, configuration changes
- ✅ **Phase 3 — `no_std + Embassy`**: Complete
  - Embassy with UDP transport validated
  - Same core logic runs in both validation and Embassy
  - Abstraction boundaries proven in most constrained environment
- 🔄 **Phase 4 — Raft Advanced Features**: In Progress (85% complete)
  - ✅ Single-Server configuration changes (fully implemented and tested)
  - ✅ Configuration tracking with dynamic quorum calculation
  - ✅ Safe add/remove server APIs with validation
  - 🔲 Joint Consensus for multi-server changes (~2-3 weeks remaining)
  - 🔲 Read-only queries with leader leases
  - 🔲 Leadership transfer
- 🔲 **Phase 5 — Cloud-Native (AWS)**: Planned (Tokio runtime, gRPC, Kubernetes deployment)

---

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
