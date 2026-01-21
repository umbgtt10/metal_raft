# ADR-R3: Deterministic and Imperative Test Harness

**Status:** Accepted
**Date:** 2026-01-16

---

## Context

Distributed algorithms are notoriously difficult to validate due to:
- non-deterministic scheduling
- real-time dependencies
- infrastructure variability

Traditional approaches that rely on:
- real clocks
- real networks
- real storage
- asynchronous integration tests

lead to:
- flaky tests
- irreproducible failures
- poor coverage of rare edge cases
- difficulty reasoning about algorithmic correctness

Additionally, declarative or purely property-based testing alone often obscures **causal intent** in complex, stateful protocols like Raft.

Given the correctness-critical nature of Raft, a more explicit and controllable testing strategy was required.

---

## Decision

**Raft algorithm correctness is validated using a deterministic, infrastructure-free, and imperative test harness**, based on:

- simulated time
- simulated message delivery
- explicit control over event ordering
- imperative test scenarios written using an **Arrange / Act / Assert** structure

Tests are written as **explicit, step-by-step scenarios**, conceptually similar to Gherkin-style specifications, but expressed imperatively in code.

Each test:
- **Arranges** the initial cluster state and failure conditions
- **Acts** by delivering messages, advancing time, or triggering events
- **Asserts** invariants, safety properties, and observable outcomes

The algorithmic core is driven entirely by controlled inputs and produces observable effects that are validated directly.

No external infrastructure is required for correctness testing.

---

## Consequences

### Positive

- **Deterministic and reproducible tests**
  Failures can be reproduced exactly and debugged reliably.

- **Clear causal reasoning**
  The Arrange / Act / Assert structure makes test intent explicit and readable.

- **Exhaustive edge-case coverage**
  Message reordering, delays, drops, node crashes, and restarts can be explored systematically.

- **Fast feedback**
  Tests execute quickly without reliance on networking, I/O, or wall-clock time.

- **Executable specifications**
  Tests act as precise, executable descriptions of Raft behavior under specific conditions.

### Trade-offs

- Requires upfront investment in custom test infrastructure
- Tests are more verbose than black-box integration tests
- Some real-world integration issues must still be validated separately

These trade-offs are intentional and acceptable given the importance of correctness.

---

## Alternatives Considered

- **Integration-only testing**
  Rejected due to non-determinism, flakiness, and limited coverage of edge cases.

- **Pure property-based testing**
  Rejected as insufficient for expressing complex, multi-step protocol scenarios and causal dependencies.

- **Formal verification**
  Rejected due to tooling cost, maintenance burden, and limited practical payoff relative to project goals.

---

## Revisit Conditions

This decision should be revisited only if:
- formal verification is adopted as a primary validation strategy, or
- deterministic simulation tooling becomes obsolete or impractical

Until then, deterministic and imperative testing remains the primary correctness strategy for the Raft core.
