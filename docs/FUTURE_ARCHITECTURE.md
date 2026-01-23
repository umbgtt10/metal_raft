# Future Architecture: Pure Algorithm + Execution Layer

**Status**: Design Exploration (Intentionally Not Implemented)
**Date**: January 23, 2026
**Context**: Architectural discussion on achieving true algorithm swappability while maintaining 100% monomorphization

**Project Decision**: MetalRaft is a completed proof-of-concept. This architecture will **not be implemented in this codebase**. These insights are documented for application to future greenfield projects where the correct foundation can be established from day one.

## Executive Summary

Through design exploration, we discovered that making the consensus algorithm truly swappable (at the same level as Storage and Transport) requires a **three-layer architecture**:

1. **Pure Algorithm Layer**: Stateless logic that reads inputs and emits actions
2. **Execution Layer**: Orchestrator that coordinates between algorithm and infrastructure
3. **Infrastructure Layer**: Owns all I/O resources (Storage, Transport, StateMachine, etc.)

This architecture pattern is **identical to modern blockchain designs** (Ethereum, Cosmos, Polkadot), validating its correctness as the canonical pattern for replicated state machines.

---

## Current Architecture Limitations

### Current Design

```
RaftNode<Storage, Transport, StateMachine, ...>
├── Owns: Storage, Transport, StateMachine (I/O)
├── Owns: Components (mixed state + logic)
└── MessageHandler: Orchestrates and performs I/O directly
```

**Limitations:**
- ❌ Algorithm (Raft logic) is tightly coupled to I/O
- ❌ Cannot swap algorithm independently of infrastructure
- ❌ Hard to test algorithm without mocking I/O
- ❌ Difficult to formally verify (side effects throughout)
- ❌ Algorithm not at same abstraction level as Storage/Transport

### Why Storage/Transport Are Swappable But Algorithm Isn't

Current type signature:
```rust
RaftNode<Storage, Transport, StateMachine, ...>
```

- ✅ `Storage` is a type parameter → swappable
- ✅ `Transport` is a type parameter → swappable
- ❌ Algorithm (Raft) is hardcoded → NOT swappable

To make algorithm swappable:
```rust
ConsensusNode<Algorithm, Storage, Transport, StateMachine, ...>
//             ^^^^^^^^^ Algorithm should be here!
```

But this requires breaking the coupling between algorithm and I/O.

---

## Proposed Architecture: Three Layers

### Layer 1: Pure Algorithm

**Characteristics:**
- No I/O dependencies
- Pure state machine: `(State, Event, Storage_Read) → (State', Actions)`
- Reads from Storage/StateMachine (immutable borrows)
- Emits Actions (explicit side effects)
- 100% testable without mocks

**Example:**
```rust
pub struct RaftAlgorithm<C, M, CCC> {
    // Pure state only
    id: NodeId,
    role: NodeState,
    current_term: Term,
    voted_for: Option<NodeId>,

    // Component state (no I/O)
    election_state: ElectionState<C>,
    replication_state: ReplicationState<M>,
    config_state: ConfigState<C>,
    snapshot_state: SnapshotState,
    lease_state: LeaseState,
}

impl RaftAlgorithm {
    /// Pure function: reads inputs, emits actions
    pub fn on_event<S, SM, AS>(
        &mut self,
        event: Event,
        storage: &S,        // Read-only input
        state_machine: &SM, // Read-only input
        sink: &mut AS,      // Action output
    ) {
        match event {
            Event::Message { from, msg } => {
                // Read from storage
                let term = storage.current_term();
                let last_log_index = storage.last_log_index();

                // Pure logic
                let should_vote = self.should_grant_vote(term, last_log_index);

                // Emit actions (no direct I/O)
                if should_vote {
                    sink.emit(Action::SetTerm(term));
                    sink.emit(Action::SetVotedFor(Some(from)));
                    sink.emit(Action::SendMessage { to: from, msg: response });
                }
            }
        }
    }
}
```

### Layer 2: Action Types

**All side effects as explicit data:**
```rust
pub enum Action<P, L, CC> {
    // Storage actions
    SetTerm(Term),
    SetVotedFor(Option<NodeId>),
    AppendLogEntries { entries: L },
    SaveSnapshot { metadata: SnapshotMetadata, data: Vec<u8> },
    TruncateLog { from_index: LogIndex },

    // Transport actions
    SendMessage { to: NodeId, msg: RaftMsg<P, L, CC> },
    BroadcastMessage { msg: RaftMsg<P, L, CC> },

    // State machine actions
    ApplyCommand { index: LogIndex, payload: P },
    RestoreFromSnapshot { data: Vec<u8> },

    // Timer actions
    StartElectionTimer,
    StartHeartbeatTimer,
    StopHeartbeatTimer,

    // Observer actions
    NotifyLeaderChange { new_leader: Option<NodeId>, term: Term },
    NotifyCommitAdvanced { commit_index: LogIndex },
    NotifyLeaseGranted,
    NotifyLeaseRevoked,
}
```

### Layer 3: Infrastructure

**Owns all I/O resources:**
```rust
pub struct Infrastructure<S, T, SM, TS, O, CLK> {
    pub storage: S,
    pub transport: T,
    pub state_machine: SM,
    pub timer_service: TS,
    pub observer: O,
    pub clock: CLK,
}

impl Infrastructure {
    /// Implements ActionSink - executes actions
    fn emit(&mut self, action: Action) {
        match action {
            Action::SetTerm(term) => {
                self.storage.set_current_term(term);
            }
            Action::SendMessage { to, msg } => {
                self.transport.send(to, msg);
            }
            Action::ApplyCommand { payload, .. } => {
                self.state_machine.apply(&payload);
            }
            Action::StartElectionTimer => {
                self.timer_service.reset_election_timer();
            }
            // ... execute all actions
        }
    }
}
```

### Layer 4: Execution Layer (Orchestrator)

**Coordinates between algorithm and infrastructure:**
```rust
pub struct ExecutionLayer<A, S, T, SM, TS, O, CLK> {
    algorithm: A,
    infrastructure: Infrastructure<S, T, SM, TS, O, CLK>,
}

impl ExecutionLayer {
    pub fn execute_event(&mut self, event: Event) {
        // Split self to avoid circular dependency
        let Self {
            ref mut algorithm,
            ref mut infrastructure,
        } = *self;

        // Algorithm reads from infrastructure and emits actions
        algorithm.on_event(
            event,
            &infrastructure.storage,
            &infrastructure.state_machine,
            infrastructure,  // Infrastructure IS the ActionSink
        );
    }
}
```

### Layer 5: Public API

**Clean top-level interface:**
```rust
pub struct ConsensusNode<A, S, T, SM, TS, O, CLK> {
    execution: ExecutionLayer<A, S, T, SM, TS, O, CLK>,
}

impl ConsensusNode {
    pub fn on_event(&mut self, event: Event) {
        self.execution.execute_event(event);
    }

    pub fn submit_command(&mut self, payload: Payload) -> Result<LogIndex, Error> {
        self.execution.submit_command(payload)
    }
}
```

---

## Benefits of New Architecture

### 1. True Swappability

```rust
// Swap algorithms like you swap storage
let raft = RaftAlgorithm::new(...);
let paxos = PaxosAlgorithm::new(...);
let epaxos = EPaxosAlgorithm::new(...);

let node: ConsensusNode<RaftAlgorithm, ...> = ConsensusNode::new(raft, infra);
let node: ConsensusNode<PaxosAlgorithm, ...> = ConsensusNode::new(paxos, infra);
```

Algorithm is now at the **same abstraction level** as Storage and Transport.

### 2. Zero-Cost Abstractions

Still 100% monomorphized:
```rust
ConsensusNode<RaftAlgorithm<C, M>, InMemoryStorage, UdpTransport, ...>
```

No `dyn Trait`, no vtables, full compile-time optimization.

### 3. Testability

```rust
// Test algorithm with simple in-memory storage
let mut algo = RaftAlgorithm::new(...);
let storage = MockStorage::new();
let actions = Vec::new();

algo.on_event(event, &storage, &state_machine, &mut actions);

// Assert on actions without any I/O
assert_eq!(actions[0], Action::SetTerm(5));
assert_eq!(actions[1], Action::SendMessage { ... });
```

No mocking frameworks needed - pure function testing.

### 4. Formal Verification

Algorithm becomes a pure state machine:
```
State × Event × Storage_View → State' × Actions
```

This is **formally verifiable** using tools like TLA+, Coq, or Lean.

### 5. Observability

All side effects are explicit `Action` values:
- Can log actions before execution
- Can defer/batch actions
- Can trace causality
- Can replay from action log

### 6. Component Transformation

Current components become pure state:

| Component | Current | After |
|-----------|---------|-------|
| MessageHandler | Orchestration + I/O | **Deleted** (becomes RaftAlgorithm) |
| ElectionManager | State + logic + timer I/O | → ElectionState (pure) |
| LogReplicationManager | State + logic | → ReplicationState (pure) |
| ConfigChangeManager | State + validation | → ConfigState (pure) |
| SnapshotManager | State + threshold | → SnapshotState (pure) |
| LeaderLease | State + clock I/O | → LeaseState (pure) |

All logic moves into `RaftAlgorithm`, components become pure data.

---

## The Blockchain Connection

This architecture is **identical** to modern blockchain designs:

### Ethereum 2.0
```
Beacon Chain (Consensus) → Execution Layer → EVM (World State)
```

### Cosmos/Tendermint
```
Tendermint (Consensus) → ABCI → CosmWasm (World State)
```

### Polkadot
```
BABE+GRANDPA (Consensus) → Runtime → Wasm (World State)
```

### MetalRaft (Proposed)
```
RaftAlgorithm (Consensus) → ExecutionLayer → StateMachine (World State)
```

### Terminology Mapping

| Blockchain Term | Raft Term | Purpose |
|-----------------|-----------|---------|
| Consensus Protocol | `RaftAlgorithm` | Pure agreement logic |
| Execution Layer | `ExecutionLayer` | Orchestrator |
| World State | `StateMachine` | Application state |
| Block Storage | `Storage` | Persistent log |
| Network Layer | `Transport` | Communication |
| Transaction | `Action` | State transition command |

**Conclusion**: This pattern isn't specific to Raft or blockchains - it's the **canonical architecture for deterministic replicated state machines**.

---

## Why This Pattern Emerges

It's not coincidence - it's **necessity** driven by fundamental requirements:

1. **Determinism**: Consensus must be pure to guarantee identical results on all replicas
2. **Testability**: Pure algorithms can be verified without I/O
3. **Separation of Concerns**: Consensus, storage, and application state are orthogonal
4. **Observability**: Explicit actions make state transitions auditable
5. **Performance**: Execution layer can batch/optimize I/O
6. **Formal Verification**: Pure state machines are provable

Any system with these requirements will converge to this architecture.

---

## Implementation Challenges

### 1. Action Allocation

**Problem**: `Vec<Action>` requires allocation.

**Solution**: Use ActionSink pattern (zero allocation):
```rust
pub trait ActionSink<P> {
    fn emit(&mut self, action: Action<P>);
}

// Algorithm emits actions as produced
algo.on_event(event, storage, sink);
```

### 2. Borrow Checker

**Problem**: Circular dependency if algorithm and infrastructure in same struct.

**Solution**: Execution layer with split borrows:
```rust
let Self { algorithm, infrastructure } = *self;
algorithm.on_event(event, &infrastructure.storage, infrastructure);
```

### 3. Type Complexity

**Problem**: Many type parameters.

**Solution**: Type aliases and builder pattern:
```rust
type RaftNode<S, T> = ConsensusNode<
    RaftAlgorithm<Vec16, Map16, ConfigChange16>,
    S, T, KeyValueStore, ...
>;
```

---

## Migration Path (Not Implemented)

If this architecture were pursued, migration would be:

### Phase 1: Extract Actions
1. Define `Action` enum with all side effects
2. Add `ActionSink` trait
3. Components emit actions instead of direct I/O

### Phase 2: Purify Algorithm
1. Create `RaftAlgorithm` struct with pure state
2. Move MessageHandler logic into RaftAlgorithm methods
3. Algorithm reads from Storage (immutable) and emits Actions

### Phase 3: Create Execution Layer
1. Extract Infrastructure struct
2. Create ExecutionLayer as orchestrator
3. Infrastructure implements ActionSink

### Phase 4: Update Components
1. Components become pure state structs
2. Remove I/O dependencies from components
3. Move component logic into RaftAlgorithm

### Phase 5: Public API
1. Create ConsensusNode wrapper
2. Update tests to new architecture
3. Update documentation

**Estimated effort**: 4-6 weeks for full refactoring with comprehensive testing.

---

## Future Directions

### Combining Generics and Dynamic Dispatch

The next evolution could **strategically combine** monomorphization with dynamic dispatch:

```rust
// Hot path: Monomorphized for performance
pub struct ConsensusNode<A: ConsensusAlgorithm, S: Storage> {
    algorithm: A,  // Compile-time dispatch
    storage: S,    // Compile-time dispatch
    transport: Box<dyn Transport>,  // Runtime dispatch (cold path)
    observer: Box<dyn Observer>,    // Runtime dispatch (telemetry)
}
```

**Principle**:
- **Generics** for hot path (algorithm, storage) → zero-cost
- **Dynamic dispatch** for cold path (transport, observer) → flexibility

This balances:
- ✅ Performance where it matters (consensus logic)
- ✅ Simplicity for non-critical paths (I/O, observability)
- ✅ Reduced cognitive load (fewer type parameters)
- ✅ Faster compile times

**Generic abstractions signal hard invariants** (compile-time contracts that cannot be violated).
**Dynamic dispatch signals soft boundaries** (runtime flexibility points without breaking core guarantees).

---

## Conclusion

This architecture represents the **optimal design for replicated state machines**, independently discovered by both Raft developers (this exploration) and blockchain engineers (Ethereum, Cosmos, Polkadot).

The pattern emerges from first principles:
- Pure algorithm for determinism and verifiability
- Execution layer for orchestration
- Infrastructure layer for I/O
- Actions for explicit side effects

**Implementation Decision**: While validated through design exploration, this architecture will **not be implemented in MetalRaft**. The current codebase successfully serves its purpose as a proof-of-concept demonstrating Raft correctness and multi-environment portability. Retrofitting this architecture would require rewriting ~80% of the code (6-8 weeks effort) on a foundation that would still carry legacy design decisions.

**The right approach**: Apply these learnings to a future greenfield project that starts with:
- Pure algorithm layer from day one
- Action-based side effects throughout
- Execution layer as primary orchestrator
- Strategic mix of generics (hot path) and dynamic dispatch (cold path)
- Designed for formal verification from the start

This document serves as the architectural blueprint for that future work.

---

## References

- Diego Ongaro's Raft Thesis (2014) - consensus algorithm
- Ethereum 2.0 Specification - Beacon Chain + Execution Layer
- Cosmos ABCI Documentation - Tendermint + Application Layer
- Polkadot Architecture - BABE/GRANDPA + Runtime
- TLA+ Specification of Raft - formal verification approach
