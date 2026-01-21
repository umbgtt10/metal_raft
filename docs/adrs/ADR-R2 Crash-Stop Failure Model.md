# ADR-R2: Crash-Stop Failure Model

**Status:** Accepted
**Date:** 2026-01-16

---

## Context

Distributed systems must explicitly define their **failure model**.

Raft, by design, assumes a **crash-stop failure model**, where nodes may:
- crash
- restart
- lose volatile state

…but do **not**:
- behave arbitrarily
- send malicious or inconsistent messages
- violate protocol semantics intentionally

Failing to state the failure model leads to:
- ambiguous guarantees
- incorrect assumptions
- false confidence in safety properties

This decision must be explicit.

---

## Decision

**The Raft core assumes a crash-stop failure model and does not attempt to handle Byzantine faults.**

Specifically:
- nodes may crash and restart
- messages may be delayed, duplicated, or reordered
- nodes do not act maliciously or arbitrarily

Byzantine behavior is **explicitly out of scope**.

---

## Consequences

### Positive

- **Algorithmic simplicity**
  Enables clear reasoning about safety and liveness properties.

- **Reduced complexity**
  Avoids cryptography, quorum inflation, and heavy validation logic.

- **Faithful alignment with Raft’s design**
  Preserves the guarantees described in the original Raft paper.

### Trade-offs

- The system is **not safe** under Byzantine faults.
- Malicious nodes can violate safety if introduced.

These trade-offs are intentional and documented.

---

## Alternatives Considered

- **Byzantine Fault Tolerance (BFT)**
  Rejected due to:
  - significantly higher complexity
  - different algorithmic requirements
  - mismatch with Raft’s design goals

- **Partial Byzantine mitigation**
  Rejected as misleading and insufficient.

---

## Revisit Conditions

This decision should be revisited only if:
- the project explicitly targets adversarial environments, or
- a different consensus algorithm is adopted

Until then, crash-stop is the authoritative failure model.
