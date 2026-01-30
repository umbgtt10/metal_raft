# MetalRaft — A Multi-Environment, Correctness-First Raft Implementation

## Overview

This project implements the **Raft consensus algorithm** once, realizes it on `no_std + Embassy`, and allows the possibility to implement it on a production-grade AWS/Kubernetes deployment.

The initial inspiration and methodological foundation for this work comes from the **MIT distributed systems labs (6.824 / 6.5840)**, particularly the Raft exercise. The project builds on those ideas but deliberately extends them toward stronger abstraction boundaries, multi-environment realizations, and production-grade operability.

## Objectives

  1) Implement Raft in a technology-agnostic core ([docs/adrs/ADR-R1%20Technology-Agnostic%20Algorithmic%20Core.md](docs/adrs/ADR-R1%20Technology-Agnostic%20Algorithmic%20Core.md))
  2) Implement Raft with a generic-only architecture (no dynamic dispatching) and show the limits of this approach ([docs/adrs/ADR-R5 Monomorphization-First Architecture.md](docs/adrs/ADR-R5%20Monomorphization-First%20Architecture.md))
  3) Validate correctness through a deterministic, adversarial test harness ([validation/README.md](validation/README.md))
  4) Realize Raft on `no_std + Embassy` for embedded targets ([embassy/README.md](embassy/README.md))
  5) Demonstrate that the same core can be realized on any platform (e.g., Tokio + gRPC + persistent storage)

## Project Structure

```
metal_raft/
  core/               # no_std Raft algorithm (frozen logic)
  validation/         # std-based deterministic simulator & test harness
  embassy/            # no_std + Embassy realization (embedded target)
  docs/               # Architecture Decision Records and implementation plans
  scripts/            # Test automation script
  test-utils/         # Shared test utilities
```

## Features

MetalRaft implements a comprehensive Raft consensus algorithm with:
- **Core protocol:** Leader election, log replication, safety guarantees
- **Log compaction & snapshots:** Automatic snapshots with bounded memory
- **Pre-Vote Protocol:** Term inflation prevention
- **Dynamic membership:** Single-server configuration changes
- **Lease-based linearizable reads:** High-performance read optimization

See [core/README.md](core/README.md) for detailed feature documentation and implementation status.

## Validation

The Raft core is validated through 232 comprehensive tests (156 core unit + 76 validation integration) using deterministic and time-based test harnesses. Tests cover all safety properties, adversarial scenarios (partitions, crashes, message drops), and multi-environment execution.

**Multi-Environment Validation:**
- ✅ Deterministic simulator (full control over time and message delivery)
- ✅ Time-based simulator (realistic timing with randomization)
- ✅ Embassy embedded (5-node QEMU cluster with UDP networking)

See [validation/README.md](validation/README.md) for complete testing documentation.

## Non-Goals

This project intentionally does **not** aim to:

* implement Byzantine fault tolerance
* maximize throughput
* compete with production systems like etcd
* support large dynamic clusters in embedded targets

## Status

POC completed and core features implemented. See individual READMEs for feature completeness and validation status.

## Issues & Limitations

See [docs/ISSUES_AND_LIMITATIONS.md](docs/ISSUES_AND_LIMITATIONS.md) for detailed discussion of:
- **Generic Explosion**: High number of type parameters (~11) affecting ergonomics and compile times
- **Missing Abstraction Layer**: Algorithm not separated from execution infrastructure

## Possible Next Steps

* Fix missing abstraction issue by refactoring the codebase to separate algorithm and execution layers
* Implement more features such as joint consensus and leadership transfer
* Implement a production-grade realization on AWS/Kubernetes
* Question the monomorphization-first architecture and explore hybrid approaches

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
