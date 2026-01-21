# ADR-R4: Explicit Separation of Safety and Liveness

**Status:** Accepted
**Date:** 2026-01-16

---

## Context

In distributed consensus algorithms, **safety** and **liveness** are fundamentally different classes of properties:

- **Safety**: something bad must *never* happen
- **Liveness**: something good must *eventually* happen

Many implementations implicitly conflate the two by:
- assuming progress without stating assumptions
- encoding liveness expectations into safety logic
- relying on real-time behavior or scheduler fairness

This conflation leads to:
- weakened safety guarantees
- ambiguous correctness claims
- tests that pass by accident rather than by construction

Given the correctness-critical nature of Raft, this ambiguity is unacceptable.

---

## Decision

**Safety and liveness are treated as explicitly separate concerns.**

- **Safety properties are unconditional**
  They must hold regardless of timing, message ordering, crashes, or lack of progress.

- **Liveness properties are conditional**
  They are validated only under explicitly stated assumptions and are not implied by default.

The Raft core prioritizes safety over progress and will not violate safety invariants in an attempt to guarantee liveness.

---

## Consequences

### Positive

- **Strong safety guarantees**
  Core invariants (e.g. log consistency, term monotonicity, committed entry stability) are enforced unconditionally.

- **Clear correctness claims**
  The implementation makes no implicit promises about progress without stated assumptions.

- **Architectural clarity**
  Safety logic is not polluted with timing heuristics or progress-driven shortcuts.

- **Test honesty**
  Tests clearly distinguish between:
  - unconditional safety validation
  - conditional liveness scenarios

### Trade-offs

- The system may legitimately stall under certain conditions
- Progress is not guaranteed unless assumptions are explicitly introduced
- Some behaviors may appear incomplete without liveness scaffolding

These trade-offs are intentional and explicitly documented.

---

## Safety Properties (Non-Exhaustive)

Examples of safety properties that must always hold:

- At most one leader per term
- Log entries are consistent across nodes (prefix property)
- Committed entries are never rolled back
- State machine commands are applied in order

These properties must hold even if the system makes no further progress.

---

## Liveness Properties (Conditional)

Examples of liveness properties that are conditional:

- A leader is eventually elected
- Client commands are eventually committed

Such properties depend on assumptions such as:
- eventual message delivery
- functioning timeouts
- availability of a majority of nodes

These assumptions are not implicit and must be modeled explicitly when liveness is tested.

---

## Alternatives Considered

- **Implicit progress assumptions**
  Rejected due to ambiguity and weakened safety reasoning.

- **Encoding liveness into safety logic**
  Rejected because safety must not depend on timing or fairness.

- **Guaranteeing liveness by design**
  Rejected as dishonest without explicit environmental assumptions.

---

## Revisit Conditions

This decision should be revisited only if:
- explicit fairness or delivery assumptions are modeled and validated, or
- the project adopts a different consensus model with stronger progress guarantees

Until then, safety remains unconditional and liveness remains explicitly scoped.
