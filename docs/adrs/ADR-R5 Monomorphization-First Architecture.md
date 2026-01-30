# ADR-R5: Monomorphization-First Architecture

**Status:** Accepted
**Date:** 2026-01-30

---

## Context

The Raft implementation needed to satisfy several critical requirements:

1. **Multi-environment execution**: Run on embedded microcontrollers (`no_std`), standard applications, test harnesses, and cloud deployments
2. **Zero runtime overhead**: Consensus algorithms are performance-critical
3. **Embedded compatibility**: Support `no_std` environments with limited resources
4. **Type safety**: Enforce protocol invariants at compile time

Traditional approaches using trait objects (`dyn Trait`) introduce runtime overhead through virtual dispatch and are incompatible with `no_std` environments that lack dynamic memory allocation.

The question: **Can a complex distributed algorithm like Raft be implemented entirely through compile-time generics (monomorphization) without dynamic dispatch?**

This project serves as a **design exploration** to answer that question and understand the limits of pure monomorphization.

---

## Decision

**Implement the entire Raft core using compile-time generics (monomorphization) without dynamic dispatch (`dyn Trait`).**

All abstractions are expressed through generic trait bounds:
- `Transport<Payload, LogEntries>`
- `Storage<Payload, LogEntryCollection, ...>`
- `StateMachine<Payload>`
- `TimerService`
- `Observer`
- Collection abstractions (`NodeCollection`, `MapCollection`, etc.)

The main `RaftNode` type carries all the required type parameters, all resolved at compile time.

---

## Consequences

### Positive

**Zero Runtime Overhead:**
- No vtable lookups or indirect calls
- Compiler can fully inline across abstraction boundaries
- Dead code elimination for unused code paths
- Branch prediction optimizations

**Embedded-Friendly:**
- Minimal binary size (core is <20KB compiled)
- No heap allocations required (except for growing logs)
- Compatible with `no_std` environments
- Deterministic performance characteristics

**Type Safety:**
- All protocol invariants enforced at compile time
- Impossible to mix incompatible implementations
- Compiler catches configuration errors early
- Self-documenting constraints through trait bounds

**Testability:**
- Easy to instantiate with mock implementations
- No hidden state or global variables
- Fully deterministic behavior
- Zero untestable dependencies

### Negative

**Type Parameter Proliferation:**
- `RaftNode` has ~11 type parameters
- Complex type signatures throughout codebase
- Example: `RaftNode<P, T, S, SM, TS, OBS, CLK, L, NC, MP, CC>`

**Cognitive Load:**
- Type signatures can obscure intent
- Increased mental overhead when reading code
- Steeper learning curve for contributors
- Error messages become verbose

**Compile Time Impact:**
- Each unique type combination generates new code
- Longer compile times as instantiations multiply
- Increased binary size with many instantiations
- Reduced IDE responsiveness (type inference)

**Limited Runtime Flexibility:**
- Cannot change implementations at runtime
- All choices must be made at compile time
- Difficult to support plugin architectures
- Hard to swap algorithms dynamically

---

## Trade-offs Accepted

This decision intentionally prioritizes:
- **Performance over ergonomics**: Zero overhead at cost of complex signatures
- **Embedded compatibility over flexibility**: Compile-time choices over runtime plugins
- **Type safety over simplicity**: Compiler-enforced contracts over simpler code
- **Design exploration over production ergonomics**: Push limits to learn, not optimize for ease

For a production system prioritizing developer ergonomics and operational flexibility, a strategic hybrid approach would be more appropriate. See [ISSUES_AND_LIMITATIONS.md](../ISSUES_AND_LIMITATIONS.md) for discussion of the generic explosion problem and potential solutions.

---

## Revisit Conditions

This decision should be revisited when:

1. **Implementing next-generation architecture** with Pure Algorithm + Execution Layer design
2. **Type parameter count exceeds 15** and cognitive load becomes unmanageable
3. **Runtime plugin system** becomes a hard requirement
4. **Embedded targets** are no longer a priority

Until then, the pure monomorphization approach successfully demonstrates that zero-cost abstractions can scale to complex distributed algorithms.

---

## References

- [ADR-R1: Technology-Agnostic Algorithmic Core](ADR-R1%20Technology-Agnostic%20Algorithmic%20Core.md)

---

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
