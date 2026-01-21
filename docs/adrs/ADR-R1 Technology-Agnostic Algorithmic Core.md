# ADR-R1: Technology-Agnostic Algorithmic Core

**Status:** Accepted
**Date:** 2026-01-16

---

## Context

Raft is a distributed consensus algorithm whose correctness depends on **logical state transitions**, not on any particular choice of:

- transport (TCP, UDP, in-memory channels)
- storage (disk, flash, RAM)
- time source (system clock, monotonic timer, simulated time)
- async runtime or executor

Many Raft implementations tightly couple the algorithmic logic with:
- networking stacks
- wall-clock time
- persistence mechanisms
- runtime-specific primitives

This coupling makes such implementations:
- difficult to test deterministically
- impossible to run in constrained (`no_std`) environments
- brittle to refactoring and architectural evolution

Given the goals of correctness, testability, and portability, this coupling was unacceptable.

---

## Decision

**The Raft algorithmic core is implemented as a pure, technology-agnostic state machine**, isolated from:

- transport
- storage
- time
- async runtime
- scheduling

The core:
- consumes abstract events (messages, timeouts)
- produces abstract effects (send message, persist state, apply entry)
- does not perform I/O directly

All side effects are delegated to outer layers via explicit interfaces.

---

## Consequences

### Positive

- **`no_std` feasibility**
  The core can run in constrained environments (embedded, bare-metal, Embassy) without allocation, threads, or OS services.

- **Deterministic testing**
  Algorithm behavior can be validated independently of timing, networking, or runtime scheduling.

- **Portability**
  The same core logic can be reused unchanged in:
  - embedded systems
  - simulated environments
  - cloud clusters
  - different async runtimes

- **Architectural clarity**
  Algorithm correctness is cleanly separated from system integration concerns.

### Trade-offs

- Requires additional abstraction layers
- Slightly more verbose integration code
- Some performance optimizations must live outside the core

These trade-offs are intentional and acceptable.

---

## Alternatives Considered

- **Runtime-specific implementation**
  Rejected due to loss of testability and portability.

- **Direct I/O inside algorithm**
  Rejected due to non-determinism and tight coupling.

- **Partial abstraction (transport only)**
  Rejected because time and persistence are equally critical to correctness.

---

## Revisit Conditions

This decision should be revisited only if:
- the Raft algorithm itself fundamentally changes, or
- the project explicitly abandons portability and deterministic testing as goals

Otherwise, this decision remains foundational.
