# Issues and Limitations

This document describes known issues and limitations in the current MetalRaft implementation.

## Limitation: Generic Explosion

### Problem

The current architecture uses **pure monomorphization** for all traits, resulting in a high number of generic type parameters (~11 parameters on core types). This creates:

1. **Cognitive Load**: Understanding function signatures requires tracking many generic parameters
2. **Complex Signatures**: Public APIs become difficult to read and use
3. **Bloated Tests**: Test code must specify all type parameters, increasing boilerplate


### Example

```rust
pub struct RaftNode<
    C: ChunkCollection,           // Log chunks
    L: LogEntryCollection,        // Individual entries
    N: NodeCollection,            // Cluster nodes
    CC: ConfigChangeCollection,   // Config changes
    M: MapCollection,             // Generic map
    S: StateMachine,              // Application state
    ST: Storage,                  // Persistence
    T: Transport,                 // Network
    TC: TimerService,             // Timers
    O: Observer,                  // Telemetry
    CLK: Clock,                   // Time source
> {
    // ...
}
```

### Consequences

- **Developer Experience**: New contributors face steep learning curve
- **API Ergonomics**: Creating and configuring a node requires specifying many types
- **Type Inference**: Compiler often cannot infer types, requiring turbofish (`::<>`) syntax
- **Documentation**: Generic parameters dominate type signatures in docs

### Why This Choice Was Made

See [ADR-R5 Monomorphization-First Architecture](adrs/ADR-R5%20Monomorphization-First%20Architecture.md) for the rationale behind pure monomorphization. The decision prioritizes:
- Zero-cost abstractions (no dynamic dispatch overhead)
- Static verification (compile-time trait bounds)
- `no_std` compatibility (no `Box<dyn Trait>`)
- Maximum performance (inlined trait methods)

### Potential Solutions

1. **Strategic Dynamic Dispatch**: Use `Box<dyn Trait>` for cold paths (Transport, Observer) while keeping hot paths (algorithm logic) monomorphized
2. **Type Aliases**: Provide common type combinations with meaningful names
3. **Builder Pattern**: Hide complexity behind fluent configuration API
4. **Greenfield Rewrite**: Design from scratch with generic explosion in mind

---

## Issue: Missing Abstraction Layer

### Problem

The current architecture **tightly couples** the Raft consensus algorithm with its execution infrastructure. The algorithm is not swappable or reusable like the `Storage` and `Transport` traits.

### What This Means

- **Algorithm Logic**: Embedded directly in `RaftNode` and component managers (`MessageHandler`, `ElectionManager`, etc.)
- **Side Effects**: I/O operations (timer calls, storage writes, transport sends) mixed with pure logic
- **State Management**: Consensus state and infrastructure dependencies intertwined
- **Testing**: Requires full infrastructure even when testing pure algorithm logic

### What a Separated Architecture Would Look Like

```rust
// Pure algorithm - no I/O dependencies
pub struct RaftAlgorithm<C, L, N> {
    state: RaftState,
    log: L,
    nodes: N,
    // ... pure state only
}

impl<C, L, N> RaftAlgorithm<C, L, N> {
    // Pure function: returns actions to execute
    pub fn on_message(&mut self, msg: Message) -> Vec<Action> {
        // Compute state transitions
        // Return actions (send message, start timer, write log)
    }
}

// Execution layer - orchestrates I/O
pub struct ExecutionLayer<S, T, TS> {
    storage: S,
    transport: T,
    timers: TS,
}

impl<S, T, TS> ExecutionLayer<S, T, TS> {
    // Executes actions produced by algorithm
    pub fn execute(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::SendMessage(to, msg) => self.transport.send(to, msg),
                Action::StartTimer(duration) => self.timers.start(duration),
                Action::WriteLog(entry) => self.storage.append(entry),
            }
        }
    }
}
```

### Why This Wasn't Implemented

MetalRaft successfully serves its purpose as a **proof-of-concept** demonstrating:
- ✅ Raft correctness across diverse test scenarios
- ✅ Multi-environment portability (`std`, `no_std`, embedded)
- ✅ Embedded-like deployment (RP2040 microcontroller)

**Decision**: Accept this limitation in the current codebase. Apply learnings to future greenfield projects that can design for algorithm/execution separation from day one.

---

## Summary

Both limitations stem from architectural decisions made early in development:

1. **Generic Explosion** → consequence of prioritizing zero-cost abstractions and `no_std` compatibility
2. **Missing Abstraction** → consequence of building Raft as an integrated system rather than separable algorithm + execution layers
