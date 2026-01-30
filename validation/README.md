# Raft Validation: Testing Infrastructure & Coverage

## Overview

This package (`raft-validation`) provides comprehensive testing infrastructure for the MetalRaft consensus implementation. The validation strategy follows **ADR-R3: Deterministic, Imperative Test Harness** and validates correctness through both deterministic simulation and time-based testing.

**Test Coverage: 232 tests (156 core unit + 76 validation integration)**

## Test Infrastructure

### Two Testing Modes

#### 1. Timeless Testing (Deterministic)
- Fully deterministic, no wall-clock time
- Total control over message delivery order
- Simulated network with partitions, drops, reordering
- Crash/restart modeling
- Perfect reproducibility

**Implementation:** `TimelessTestCluster` in [src/timeless_test_cluster.rs](src/timeless_test_cluster.rs)

#### 2. Timefull Testing (Realistic)
- Wall-clock based timing
- Randomized election timeouts
- Realistic network latency
- Validates timing-dependent behavior
- Ensures algorithm works with real time

**Implementation:** `TimefullTestCluster` in [src/timefull_test_cluster.rs](src/timefull_test_cluster.rs)

### Test Philosophy (ADR-R3)

All tests follow **Arrange / Act / Assert** structure:
- **Arrange**: Set up cluster state and failure conditions
- **Act**: Deliver messages, advance time, trigger events
- **Assert**: Validate invariants and observable outcomes

Tests are **executable specifications** of Raft behavior under specific conditions.

## Test Organization

### Core Unit Tests (156 tests in `core/tests/`)

**Component Tests (136 tests):**
- [election_manager_tests.rs](../core/tests/election_manager_tests.rs) - 24 tests
- [log_replication_manager_tests.rs](../core/tests/log_replication_manager_tests.rs) - 40 tests
- [snapshot_manager_tests.rs](../core/tests/snapshot_manager_tests.rs) - 21 tests
- [config_change_manager_tests.rs](../core/tests/config_change_manager_tests.rs) - 25 tests
- [role_transition_manager_tests.rs](../core/tests/role_transition_manager_tests.rs) - 13 tests
- [message_handler_tests.rs](../core/tests/message_handler_tests.rs) - 27 tests
- [leader_lease_tests.rs](../core/tests/leader_lease_tests.rs) - 6 tests

### Integration Tests (76 tests in `validation/tests/`)

**Election Protocol (12 tests):**
- [election/basic_leader_election_tests.rs](tests/election/basic_leader_election_tests.rs) - Basic election scenarios
- [election/pre_vote_tests.rs](tests/election/pre_vote_tests.rs) - 6 dedicated Pre-Vote tests
- [election/election_with_log_restriction.rs](tests/election/election_with_log_restriction.rs) - Log-based election safety
- [election/timed_election_tests.rs](tests/election/timed_election_tests.rs) - Timing-dependent scenarios

**Replication & Safety (25 tests):**
- [replication/client_payload_replication_tests.rs](tests/replication/client_payload_replication_tests.rs)
- [replication/conflict_resolution_tests.rs](tests/replication/conflict_resolution_tests.rs)
- [replication/commit_index_advancement_tests.rs](tests/replication/commit_index_advancement_tests.rs)
- [replication/follower_far_behind.rs](tests/replication/follower_far_behind.rs)
- [replication/append_entry_idempotency.rs](tests/replication/append_entry_idempotency.rs)
- [replication/cannot_commit_old_entries.rs](tests/replication/cannot_commit_old_entries.rs)
- [replication/rapid_sequential_command.rs](tests/replication/rapid_sequential_command.rs)
- [replication/lease_integration_tests.rs](tests/replication/lease_integration_tests.rs) - 3 lease tests

**Snapshots & Compaction (15 tests):**
- [snapshots/snapshot_creation_protocol_tests.rs](tests/snapshots/snapshot_creation_protocol_tests.rs)
- [snapshots/snapshot_infrastructure_tests.rs](tests/snapshots/snapshot_infrastructure_tests.rs)
- [snapshots/install_snapshot_candidate_tests.rs](tests/snapshots/install_snapshot_candidate_tests.rs)

**Fault Tolerance (10 tests):**
- [fault_tolerance/crash_recovery_with_snapshots_tests.rs](tests/fault_tolerance/crash_recovery_with_snapshots_tests.rs)
- [fault_tolerance/leader_steps_down.rs](tests/fault_tolerance/leader_steps_down.rs)

**Membership Changes (8 tests):**
- [membership/config_api_tests.rs](tests/membership/config_api_tests.rs)

**State Machine (6 tests):**
- [state_machine/apply_state_machine_tests.rs](tests/state_machine/apply_state_machine_tests.rs)
- [state_machine/state_machine_safety_tests.rs](tests/state_machine/state_machine_safety_tests.rs)

## Test Coverage by Feature

### ✅ Core Raft Protocol (Complete)
- Leader election with randomized timeouts
- Log replication with consistency checks
- Commit index advancement (quorum-based)
- State machine application (in-order, idempotent)
- All safety guarantees validated

### ✅ Pre-Vote Protocol (Complete)
- 6 dedicated tests
- Term inflation prevention
- Split-vote avoidance
- Partition tolerance

### ✅ Log Compaction & Snapshots (Complete)
- Automatic snapshot creation at threshold
- InstallSnapshot RPC with chunked transfer
- Crash recovery with snapshot restoration
- Follower catch-up via snapshots
- 21 snapshot-specific tests

### ✅ Dynamic Membership (Single-Server Changes)
- Add/remove one server at a time
- Configuration validation
- Catching-up servers (non-voting until synchronized)
- 25 configuration change tests

### ✅ Lease-Based Linearizable Reads (Complete)
- Leader lease grant/revoke logic
- 9 comprehensive tests (6 unit + 3 integration)
- Safety guarantees validated

## Running Tests

### All Tests
```bash
# From workspace root
cargo test --workspace

# Or use the script
.\scripts\run_all_tests.ps1
```

### Core Unit Tests Only
```bash
cargo test -p raft-core
```

### Integration Tests Only
```bash
cargo test -p raft-validation
```

### Specific Test Suite
```bash
# Election tests
cargo test -p raft-validation election

# Snapshot tests
cargo test -p raft-validation snapshots

# Replication tests
cargo test -p raft-validation replication
```

## Test Results

All 232 tests pass deterministically in both timeless and timefull modes:
- ✅ 156 core unit tests
- ✅ 76 validation integration tests
- ✅ Zero flaky tests
- ✅ 100% reproducible failures (when injected)

## Validation Strategy

### Safety Properties Validated
1. **Election Safety**: At most one leader per term
2. **Leader Append-Only**: Leaders never overwrite entries
3. **Log Matching**: Identical entries at same index across nodes
4. **Leader Completeness**: Committed entries in all future leaders
5. **State Machine Safety**: Deterministic, in-order application

### Adversarial Scenarios Tested
- Network partitions and healing
- Message drops, delays, reordering
- Node crashes and restarts
- Split votes and term inflation
- Conflicting log entries
- Snapshot transfer failures
- Configuration change edge cases

### Multi-Environment Validation
- ✅ **Deterministic simulator** (timeless mode)
- ✅ **Time-based simulator** (timefull mode)
- ✅ **Embassy embedded** (5-node QEMU cluster)
- ✅ **Test-utils** (standard Rust with heap allocations)

**Same Raft core logic runs unchanged across all environments.**

## Contributing

When adding new features to Raft core:

1. **Write unit tests first** in `core/tests/`
2. **Add integration scenarios** in `validation/tests/`
3. **Test both timeless and timefull** modes
4. **Validate safety properties** explicitly
5. **Document test intent** clearly (Arrange/Act/Assert)

See [ADR-R3](../docs/adrs/ADR-R3%20Deterministic,%20Imperative%20Test%20Harness.md) for testing philosophy.

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0
