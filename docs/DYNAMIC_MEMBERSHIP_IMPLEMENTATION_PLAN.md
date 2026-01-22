# Dynamic Membership Implementation Plan

**Current Status:** Phase A (Single-Server Changes) ~85% Complete
**Remaining Work:** Phase A Final Polish (1 week) + Phase B (Joint Consensus Upgrade)

**Total Estimated Effort:** 2-3 weeks for Joint Consensus

---

## Overview

Dynamic membership allows adding and removing nodes from a Raft cluster without downtime. This implementation is divided into two phases:

1. **Phase A: Single-Server Changes** (Weeks 1-3) - ✅ **~70% COMPLETE** - Foundation built
2. **Phase B: Joint Consensus Upgrade** (Weeks 4-5) - ❌ **PENDING** - Full support for multi-server changes

**Key Insight:** 80% of the code is shared between both approaches. Single-Server Changes builds all the infrastructure, and Joint Consensus is primarily a quorum calculation upgrade.

**Implementation Status:**
- ✅ Configuration entry types (EntryType::ConfigChange)
- ✅ Configuration tracking in RaftNode
- ✅ ConfigChangeManager with validation
- ✅ AddServer/RemoveServer API
- ✅ Observer events for configuration changes
- ❌ Joint consensus logic (C_old,new state)
- ❌ Dual quorum calculation
- ❌ Automatic C_old,new → C_new transition

---

## Implementation Notes (2026-01-22 - Updated)

**What's Already Done (Phase A ~85%):**

Tasks 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, and 2.3 from Weeks 1-3 are implemented and tested:

1. ✅ **Configuration Entry Types** - [core/src/log_entry.rs](../core/src/log_entry.rs)
   - `EntryType::ConfigChange(ConfigurationChange)` enum
   - `ConfigurationChange::AddServer` and `RemoveServer` variants
   - Fully serialized in embassy UDP transport

2. ✅ **Configuration Tracking** - [core/src/collections/configuration.rs](../core/src/collections/configuration.rs)
   - `Configuration<C: NodeCollection>` struct
   - `contains()`, `size()`, `quorum_size()`, `has_quorum()`, `peers()` methods
   - Integrated into RaftNode via ConfigChangeManager

3. ✅ **Dynamic Quorum Calculation** - [core/src/components/log_replication_manager.rs](../core/src/components/log_replication_manager.rs)
   - `compute_median()` with leader filtering (fixed in recent refactor)
   - Configuration-based quorum for both log replication and elections
   - Respects current cluster membership dynamically

4. ✅ **ConfigChangeManager** - [core/src/components/config_change_manager.rs](../core/src/components/config_change_manager.rs)
   - `add_server()` and `remove_server()` validation
   - Tracks pending configuration changes
   - Error handling for concurrent changes, duplicate nodes, etc.
   - Integrated into MessageHandler
   - Apply logic with observer notifications
   - Catching-up servers mechanism (non-voting until synced)

5. ✅ **Observer Events** - [core/src/observer.rs](../core/src/observer.rs)
   - `configuration_change_applied()` callback

6. ✅ **Snapshot Configuration Storage** - [core/src/snapshot.rs](../core/src/snapshot.rs)
   - Configuration survives log compaction
   - Proper restoration after crash

7. ✅ **Comprehensive Testing** - [validation/tests/membership/](../validation/tests/membership/)
   - 21 test files in validation suite (144+ total tests)
   - config_api_tests.rs with full coverage
   - Crash recovery tests
   - Snapshot tests
   - All tests passing

**What Remains (Phase A - ~15%):**

1. ⚠️ **Documentation Polish** (1-2 days)
   - User-facing API documentation
   - Migration guide from static to dynamic membership
   - Best practices for adding/removing servers

2. ⚠️ **Edge Case Testing** (2-3 days)
   - More chaos/partition scenarios during config changes
   - Stress testing with rapid sequential changes
   - Multi-node failure during configuration change

**What Remains (Phase B - Joint Consensus):**

The primary remaining work is implementing **Joint Consensus** for safe multi-server changes:

- ❌ Two-configuration state tracking (C_old and C_old,new)
- ❌ Joint quorum calculation (majority from both configs)
- ❌ Automatic transition from C_old,new → C_new
- ❌ Tests for joint consensus scenarios

**Next Steps:** Skip to Phase B (Joint Consensus Upgrade) in this document for remaining implementation work.

---

## Phase A: Single-Server Changes Implementation

### Week 1: Foundation & Infrastructure

#### **Task 1.1: Configuration Entry Types** ✅ **COMPLETE**
**Goal:** Establish log entry types for configuration changes.

**Implementation Status:** ✅ Implemented in [core/src/log_entry.rs](../core/src/log_entry.rs)
```rust
// core/src/log_entry.rs
#[derive(Clone, Debug, PartialEq)]
pub enum EntryType<P> {
    Command(P),
    ConfigChange(ConfigurationChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfigurationChange {
    AddServer(NodeId),
    RemoveServer(NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry<P> {
    pub term: Term,
    pub entry_type: EntryType<P>,
}
```

**Changes Required:**
- Update `LogEntry` structure throughout codebase
- Keep `StateMachine::apply()` for commands only; handle config changes inside `RaftNode` (no trait change)
- Update serialization in:
  - `embassy/src/transport/udp/serde_raft_message.rs`
  - Any Postcard/Protobuf implementations
- Update all tests to use new `LogEntry` structure

**Testing:**
- Unit tests for serialization/deserialization
- Ensure existing tests still pass with `EntryType::Command` wrapper

---

#### **Task 1.2: Configuration Tracking in RaftNode** (2 hours)
**Goal:** Track current cluster configuration.

**Implementation:**
```rust
// core/src/configuration.rs (new file)
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration<C: NodeCollection> {
    pub members: C,
}

impl<C: NodeCollection> Configuration<C> {
    pub fn new(members: C) -> Self {
        Self { members }
    }

    pub fn contains(&self, node_id: NodeId) -> bool {
        self.members.iter().any(|id| id == node_id)
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }
}

// core/src/raft_node.rs
pub struct RaftNode<...> {
    // ... existing fields
    current_config: Configuration<C>,
    pending_config_index: Option<LogIndex>,
}
```

**Changes Required:**
- Create `core/src/configuration.rs`
- Add `current_config` field to `RaftNode`
- Initialize from `peers` in `RaftNodeBuilder`
- Add getter methods: `current_config()`, `is_member()`, `config_size()`

**Testing:**
- Unit tests for `Configuration` methods
- Verify initialization in builder

---

#### **Task 1.3: Dynamic Quorum - Log Replication** (4 hours)
**Goal:** Calculate commit quorum based on current configuration.

**Implementation:**
```rust
// core/src/log_replication_manager.rs
pub fn advance_commit_index<P, L, S, SM, C>(
    &mut self,
    storage: &S,
    state_machine: &mut SM,
    current_config: &Configuration<C>,
) where
    C: NodeCollection,
    // ... other bounds
{
    let leader_index = storage.last_log_index();
    let quorum_index = self.compute_quorum(leader_index, current_config);

    if quorum_index > self.commit_index {
        if let Some(entry) = storage.get_entry(quorum_index) {
            if entry.term == storage.current_term() {
                self.commit_index = quorum_index;
                self.apply_committed_entries(storage, state_machine);
            }
        }
    }
}

fn compute_quorum<C: NodeCollection>(
    &self,
    leader_index: LogIndex,
    config: &Configuration<C>,
) -> LogIndex {
    let mut indices: Vec<LogIndex> = Vec::new();

    // Add leader's index
    indices.push(leader_index);

    // Add match indices for all members (excluding self)
    for member in config.members.iter() {
        // Leader already added
        if let Some(match_idx) = self.match_index.get(member) {
            indices.push(match_idx);
        }
    }

    // Sort and find median
    indices.sort_unstable();
    let majority_idx = (indices.len() - 1) / 2;
    indices[majority_idx]
}
```

**Changes Required:**
- Add `current_config` parameter to `advance_commit_index()`
- Extract `compute_quorum()` method
- Update all callsites in `RaftNode` to pass config
- Remove dependency on `total_peers` count

**Testing:**
- Test with 3-node cluster, verify majority = 2
- Test with 5-node cluster, verify majority = 3
- Test quorum calculation edge cases

---

#### **Task 1.4: Dynamic Quorum - Elections** (3 hours)
**Goal:** Election quorum respects current configuration.

**Implementation:**
```rust
// core/src/election_manager.rs
pub fn handle_vote_response<C: NodeCollection>(
    &mut self,
    from: NodeId,
    term: Term,
    vote_granted: bool,
    current_term: &Term,
    role: &NodeState,
    current_config: &Configuration<C>,
) -> bool {
    if term > *current_term || *role != NodeState::Candidate
        || term != *current_term || !vote_granted {
        return false;
    }

    self.votes_received.push(from).ok();

    let total_nodes = current_config.size();
    let votes = self.votes_received.len();

    votes > total_nodes / 2  // Majority of current config
}

// Similar changes for handle_pre_vote_response
```

**Changes Required:**
- Add `current_config` parameter to vote handlers
- Replace `total_peers` with `current_config.size()`
- Update callsites in `RaftNode`
- Apply same pattern to pre-vote

**Testing:**
- Test election with 3 nodes, verify 2 votes needed
- Test election with 5 nodes, verify 3 votes needed

**End of Week 1 Checkpoint:**
- ✅ Configuration tracking infrastructure
- ✅ Dynamic quorum for commits and elections
- ✅ All existing tests passing
- ⚠️ No API to change config yet (next week)

---

### Week 2: Configuration Change Protocol

#### **Task 2.1: Apply Configuration Changes** (5 hours)
**Goal:** Leader and followers can apply configuration changes.

**Implementation:**
```rust
// core/src/raft_node.rs
// NOTE: apply_log_entry is called only for committed entries.
fn apply_log_entry(&mut self, index: LogIndex, entry: &LogEntry<P>)
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    match &entry.entry_type {
        EntryType::Command(payload) => {
            self.state_machine.apply(payload);
        }
        EntryType::ConfigChange(change) => {
            self.apply_config_change(change.clone());
        }
    }
}

// Called when new log entries are appended/received to update
// the active configuration used for quorum/elections.
fn update_active_config_from_log(&mut self, entry: &LogEntry<P>) {
    if let EntryType::ConfigChange(change) = &entry.entry_type {
        self.activate_config_change_for_quorum(change.clone());
    }
}

fn apply_config_change(&mut self, change: ConfigurationChange) {
    match change {
        ConfigurationChange::AddServer(node_id) => {
            // Add to current config
            self.current_config.members.push(node_id).ok();

            // If leader, initialize replication for new member
            if self.role == NodeState::Leader {
                self.replication.initialize_peer(node_id, &self.storage);

                // Send InstallSnapshot if log is compacted
                if self.storage.first_log_index() > 1 {
                    self.send_install_snapshot_to(node_id);
                }
            }

            self.observer.config_changed(self.id, &self.current_config);
        }
        ConfigurationChange::RemoveServer(node_id) => {
            // Remove from config
            self.current_config.members.retain(|id| id != node_id);

            // If we're being removed, step down
            if node_id == self.id && self.role == NodeState::Leader {
                self.step_down(self.current_term);
            }

            // Clean up replication state
            if self.role == NodeState::Leader {
                self.replication.remove_peer(node_id);
            }

            self.observer.config_changed(self.id, &self.current_config);
        }
    }
}
```

**Implementation in Replication Manager:**
```rust
// core/src/log_replication_manager.rs
pub fn initialize_peer<P, L, S>(
    &mut self,
    peer: NodeId,
    storage: &S,
) where
    S: Storage<Payload = P, LogEntryCollection = L>,
{
    let next_idx = storage.last_log_index() + 1;
    self.next_index.insert(peer, next_idx);
    self.match_index.insert(peer, 0);
}

pub fn remove_peer(&mut self, peer: NodeId) {
    self.next_index.remove(peer);
    self.match_index.remove(peer);
}
```

**Changes Required:**
- Modify `handle_append_entries()` to distinguish entry types
- Apply config changes only when the entry is committed (via `apply_log_entry`)
- Maintain a separate **active configuration** for quorum/elections based on the latest
    config entry in the log (committed or not), updated when entries are appended/received
- Add `initialize_peer()` and `remove_peer()` to replication manager
- Update `MapCollection` trait if needed (add `remove()` method)
- Add observer event `config_changed()`

**Testing:**
- Test adding a node (verify replication state initialized)
- Test removing a node (verify state cleaned up)
- Test leader removal (verify step-down)

---

#### **Task 2.2: Configuration Change API** (4 hours)
**Goal:** Leader can initiate configuration changes.

**Implementation:**
```rust
// core/src/raft_node.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigChangeError {
    NotLeader,
    ChangeInProgress,
    InvalidChange,
}

pub fn add_server(&mut self, node_id: NodeId) -> Result<LogIndex, ConfigChangeError>
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    if self.role != NodeState::Leader {
        return Err(ConfigChangeError::NotLeader);
    }

    if self.pending_config_index.is_some() {
        return Err(ConfigChangeError::ChangeInProgress);
    }

    if self.current_config.contains(node_id) {
        return Err(ConfigChangeError::InvalidChange);
    }

    let entry = LogEntry {
        term: self.current_term,
        entry_type: EntryType::ConfigChange(ConfigurationChange::AddServer(node_id)),
    };

    self.storage.append_entries(&[entry]);
    let index = self.storage.last_log_index();

    // Update active config for quorum/elections (commit will apply)
    self.activate_config_change_for_quorum(ConfigurationChange::AddServer(node_id));

    // Track pending change
    self.pending_config_index = Some(index);

    // Replicate to all members (old + new)
    self.send_append_entries_to_followers();

    Ok(index)
}

pub fn remove_server(&mut self, node_id: NodeId) -> Result<LogIndex, ConfigChangeError>
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    if self.role != NodeState::Leader {
        return Err(ConfigChangeError::NotLeader);
    }

    if self.pending_config_index.is_some() {
        return Err(ConfigChangeError::ChangeInProgress);
    }

    if !self.current_config.contains(node_id) {
        return Err(ConfigChangeError::InvalidChange);
    }

    let entry = LogEntry {
        term: self.current_term,
        entry_type: EntryType::ConfigChange(ConfigurationChange::RemoveServer(node_id)),
    };

    self.storage.append_entries(&[entry]);
    let index = self.storage.last_log_index();

    // Update active config for quorum/elections (commit will apply)
    self.activate_config_change_for_quorum(ConfigurationChange::RemoveServer(node_id));

    self.pending_config_index = Some(index);
    self.send_append_entries_to_followers();

    Ok(index)
}

// Clear pending flag when config change commits
fn on_commit_advanced(&mut self, new_commit: LogIndex) {
    if let Some(pending_idx) = self.pending_config_index {
        if new_commit >= pending_idx {
            self.pending_config_index = None;
        }
    }
}
```

**Changes Required:**
- Add `add_server()` and `remove_server()` public methods
- Add `ConfigChangeError` enum
- Track `pending_config_index` to prevent concurrent changes
- Clear pending flag in commit advancement logic
- Add event to `Event` enum: `ConfigChange { change: ConfigurationChange }`

**Testing:**
- Test successful add/remove
- Test rejection when not leader
- Test rejection when change in progress
- Test rejection of invalid changes (duplicate add, non-existent remove)

---

#### **Task 2.3: Snapshot Configuration Storage** (3 hours)
**Goal:** Configuration survives log compaction.

**Implementation:**
```rust
// core/src/snapshot.rs
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotMetadata {
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
    pub configuration: Vec<NodeId>,  // Simplified for serialization
}

// core/src/raft_node.rs
fn create_snapshot_internal(&mut self) -> Result<(), SnapshotError>
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    S: Storage<Payload = P, LogEntryCollection = L>,
{
    let last_applied = self.replication.last_applied();
    // ... existing snapshot creation ...

    // Include current configuration
    let config_members: Vec<NodeId> = self.current_config.members.iter().collect();

    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: last_applied,
            last_included_term,
            configuration: config_members,
        },
        data: snapshot_data,
    };

    self.storage.save_snapshot(snapshot);
    // ...
}

// Restore configuration on startup
pub(crate) fn new_from_builder(
    // ... params
) -> Self {
    // ... existing code ...

    // Restore config from snapshot
    let current_config = if let Some(snapshot) = storage.load_snapshot() {
        let mut members = C::new();
        for member in snapshot.metadata.configuration {
            members.push(member).ok();
        }
        Configuration::new(members)
    } else {
        // Initialize from peers
        Configuration::new(peers.clone())
    };

    RaftNode {
        // ... fields
        current_config,
        pending_config_index: None,
        // ...
    }
}
```

**Changes Required:**
- Add `configuration` field to `SnapshotMetadata`
- Save config when creating snapshot
- Restore config when loading snapshot in constructor
- Update serialization to handle Vec<NodeId>

**Testing:**
- Test snapshot creation includes config
- Test node restart restores config from snapshot
- Test config survives log compaction

**End of Week 2 Checkpoint:**
- ✅ Full configuration change protocol working
- ✅ Leader can add/remove servers
- ✅ Followers apply changes correctly
- ✅ Configuration survives snapshots
- ⚠️ Need more edge case handling

---

### Week 3: Edge Cases & Hardening

#### **Task 3.1: Catching-Up Servers** (4 hours)
**Goal:** New servers don't block commits forever.

**Note:** This is an optional **learner/non-voting** enhancement. If we do not
introduce learners, keep standard Raft quorum semantics and skip this task.

**Implementation:**
```rust
// core/src/log_replication_manager.rs
pub struct LogReplicationManager<M> {
    // ... existing fields
    catching_up_servers: Vec<NodeId>,  // Temporary, not in MapCollection
}

pub fn mark_server_catching_up(&mut self, node_id: NodeId) {
    if !self.catching_up_servers.contains(&node_id) {
        self.catching_up_servers.push(node_id);
    }
}

pub fn check_caught_up(&mut self, node_id: NodeId) -> bool {
    if self.catching_up_servers.contains(&node_id) {
        let match_idx = self.match_index.get(node_id).unwrap_or(0);
        if match_idx >= self.commit_index {
            // Remove from catching up
            self.catching_up_servers.retain(|id| *id != node_id);
            return true;
        }
    }
    false
}

// Modified quorum calculation
fn compute_quorum<C: NodeCollection>(
    &self,
    leader_index: LogIndex,
    config: &Configuration<C>,
) -> LogIndex {
    let mut indices: Vec<LogIndex> = Vec::new();
    indices.push(leader_index);

    for member in config.members.iter() {
        // Skip catching-up servers (they start at match_index=0)
        if self.catching_up_servers.contains(&member) {
            continue;
        }

        if let Some(match_idx) = self.match_index.get(member) {
            indices.push(match_idx);
        }
    }

    indices.sort_unstable();
    let majority_idx = (indices.len() - 1) / 2;
    indices[majority_idx]
}

// In RaftNode::apply_config_change
ConfigurationChange::AddServer(node_id) => {
    // ...existing code...
    if self.role == NodeState::Leader {
        self.replication.mark_server_catching_up(node_id);
    }
}

// In handle_append_entries_response
if success {
    // ... update match_index ...
    self.replication.check_caught_up(from);
}
```

**Changes Required:**
- (Optional) Add `catching_up_servers` tracking
- (Optional) Exclude catching-up servers from quorum until caught up
- (Optional) Mark new servers as catching up when added
- (Optional) Check and promote when match_index reaches commit_index

**Testing:**
- (Optional) Test new server with empty log doesn't block commits
- (Optional) Test new server transitions to voting once caught up
- (Optional) Test commit advancement with mixed caught-up/catching-up servers

---

#### **Task 3.2: Leader Removal Safety** (2 hours)
**Goal:** Handle leader removing itself gracefully.

**Implementation:**
```rust
// core/src/raft_node.rs
pub fn remove_server(&mut self, node_id: NodeId) -> Result<LogIndex, ConfigChangeError> {
    // ... existing validation ...

    // Special case: Leader removing itself
    if node_id == self.id {
        // Option 1: Reject (conservative)
        return Err(ConfigChangeError::LeaderRemovalNotAllowed);

        // Option 2: Allow but mark for step-down
        self.will_step_down = true;
    }

    // ... rest of implementation ...
}

// In on_commit_advanced
fn on_commit_advanced(&mut self, new_commit: LogIndex) {
    if let Some(pending_idx) = self.pending_config_index {
        if new_commit >= pending_idx {
            self.pending_config_index = None;

            // Step down if we removed ourselves
            if self.will_step_down {
                self.step_down(self.current_term);
                self.will_step_down = false;
            }
        }
    }
}
```

**Changes Required:**
- Add `ConfigChangeError::LeaderRemovalNotAllowed` variant
- OR: Add `will_step_down` flag and delay step-down until commit
- Add tests for both approaches

**Testing:**
- Test leader removal rejected (if Option 1)
- Test leader steps down after commit (if Option 2)
- Verify cluster continues with new leader

---

#### **Task 3.3: Single-Server Edge Case Tests** (6 hours)
**Goal:** Comprehensive test coverage for edge cases.

**Tests to Implement:**

1. **test_add_server_basic** (1h)
   - 3-node cluster (1, 2, 3)
   - Leader adds node 4
   - Verify: Node 4 joins, receives all log entries, can vote

2. **test_remove_server_basic** (1h)
   - 3-node cluster
   - Leader removes node 3
   - Verify: Node 3 steps down, cluster commits with 2 nodes

3. **test_concurrent_config_change_rejected** (30min)
   - Leader adds node 4
   - Before commit, try to add node 5
   - Verify: Second change rejected with `ChangeInProgress`

4. **test_non_leader_config_change_rejected** (30min)
   - Follower tries to add server
   - Verify: Rejected with `NotLeader`

5. **test_invalid_config_changes** (30min)
   - Try to add existing node
   - Try to remove non-existent node
   - Verify: Both rejected with `InvalidChange`

6. **test_partition_during_config_change** (1.5h)
   - 5-node cluster, partition into (1,2) and (3,4,5)
   - Leader in (1,2) tries to add node 6
   - Verify: Change fails to commit (no majority)
   - Heal partition, verify change completes

7. **test_snapshot_preserves_config** (1h)
   - 3-node cluster
   - Add node 4, trigger snapshot
   - Restart node 4 from snapshot
   - Verify: Node 4 knows it's in 4-node cluster

8. **test_new_server_catches_up** (1h)
   - 3-node cluster with 100 entries
   - Add node 4
   - Verify: Node 4 receives all entries via InstallSnapshot or AppendEntries
   - Verify: Commits continue during catch-up

9. **test_remove_majority_of_servers** (30min)
   - 5-node cluster
   - Remove 3 nodes sequentially
   - Verify: Each removal commits before next starts

10. **test_config_change_with_leader_crash** (1h)
    - Leader adds node 4, crashes before commit
    - New leader elected
    - Verify: Config change either completes or gets rolled back correctly

**Implementation Location:**
- `raft/sim/tests/dynamic_membership_tests.rs` (new file)

**End of Week 3 Checkpoint:**
- ✅ Single-Server Changes fully implemented
- ✅ Edge cases handled
- ✅ Comprehensive test suite passing
- ✅ Production-ready for single-server membership changes
- **🎉 Milestone: Deployable to production**

---

## Phase B: Joint Consensus Upgrade

### Week 4: Joint Consensus Implementation

#### **Task 4.1: Extend Configuration State** (2 hours)
**Goal:** Support dual configurations.

**Implementation:**
```rust
// core/src/configuration.rs
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigurationState<C: NodeCollection> {
    Stable(Configuration<C>),
    Joint {
        old: Configuration<C>,
        new: Configuration<C>,
    },
}

impl<C: NodeCollection> ConfigurationState<C> {
    pub fn is_joint(&self) -> bool {
        matches!(self, Self::Joint { .. })
    }

    pub fn is_member(&self, node_id: NodeId) -> bool {
        match self {
            Self::Stable(c) => c.contains(node_id),
            Self::Joint { old, new } => old.contains(node_id) || new.contains(node_id),
        }
    }

    pub fn all_members(&self) -> impl Iterator<Item = NodeId> + '_ {
        match self {
            Self::Stable(c) => c.members.iter().chain(std::iter::empty()),
            Self::Joint { old, new } => old.members.iter().chain(new.members.iter()),
        }
    }
}

// core/src/log_entry.rs - extend EntryType
pub enum EntryType<P> {
    Command(P),
    ConfigChange(ConfigurationChange),  // Keep for backward compat
    JointConfig { old: Vec<NodeId>, new: Vec<NodeId> },  // C_old,new
    NewConfig(Vec<NodeId>),              // Final C_new
}
```

**Changes Required:**
- Refactor `current_config: Configuration` → `configuration_state: ConfigurationState`
- Update all callsites to use `configuration_state`
- Add new entry types for joint consensus
- Update serialization

**Testing:**
- Unit tests for `ConfigurationState` methods
- Verify backward compatibility with existing Single-Server entries

---

#### **Task 4.2: Double Quorum Logic** (6 hours)
**Goal:** Implement joint consensus quorum calculation.

**Implementation:**
```rust
// core/src/log_replication_manager.rs
pub fn advance_commit_index<P, L, S, SM, C>(
    &mut self,
    storage: &S,
    state_machine: &mut SM,
    config_state: &ConfigurationState<C>,
) where
    C: NodeCollection,
{
    let leader_index = storage.last_log_index();

    let quorum_index = match config_state {
        ConfigurationState::Stable(config) => {
            // Single quorum (reuse existing code)
            self.compute_single_quorum(leader_index, config)
        }
        ConfigurationState::Joint { old, new } => {
            // Double quorum - BOTH must agree
            let old_quorum = self.compute_single_quorum(leader_index, old);
            let new_quorum = self.compute_single_quorum(leader_index, new);

            // Take minimum (most conservative)
            old_quorum.min(new_quorum)
        }
    };

    if quorum_index > self.commit_index {
        if let Some(entry) = storage.get_entry(quorum_index) {
            if entry.term == storage.current_term() {
                self.commit_index = quorum_index;
                self.apply_committed_entries(storage, state_machine);
            }
        }
    }
}

// Refactored from compute_quorum
fn compute_single_quorum<C: NodeCollection>(
    &self,
    leader_index: LogIndex,
    config: &Configuration<C>,
) -> LogIndex {
    let mut indices: Vec<LogIndex> = Vec::new();
    indices.push(leader_index);

    for member in config.members.iter() {
        if self.catching_up_servers.contains(&member) {
            continue;
        }
        if let Some(match_idx) = self.match_index.get(member) {
            indices.push(match_idx);
        }
    }

    indices.sort_unstable();
    let majority_idx = (indices.len() - 1) / 2;
    indices[majority_idx]
}
```

**Election Manager:**
```rust
// core/src/election_manager.rs
pub fn handle_vote_response<C: NodeCollection>(
    &mut self,
    from: NodeId,
    // ... params
    config_state: &ConfigurationState<C>,
) -> bool {
    // ... existing validation ...

    self.votes_received.push(from).ok();

    match config_state {
        ConfigurationState::Stable(config) => {
            let votes_needed = (config.size() / 2) + 1;
            self.votes_received.len() >= votes_needed
        }
        ConfigurationState::Joint { old, new } => {
            // Count votes in each config separately
            let old_votes = self.count_votes_in_config(&self.votes_received, old);
            let new_votes = self.count_votes_in_config(&self.votes_received, new);

            let old_needed = (old.size() / 2) + 1;
            let new_needed = (new.size() / 2) + 1;

            // Need majority in BOTH configs
            old_votes >= old_needed && new_votes >= new_needed
        }
    }
}

fn count_votes_in_config<C: NodeCollection>(
    &self,
    votes: &C,
    config: &Configuration<C>,
) -> usize {
    votes.iter().filter(|v| config.contains(*v)).count()
}
```

**Changes Required:**
- Split quorum logic into single/joint paths
- Add `compute_single_quorum()` helper (reusable)
- Update election manager for double majority
- Apply same pattern to pre-vote
- Update all callsites

**Testing:**
- Test single quorum still works (backward compat)
- Test joint quorum requires BOTH majorities
- Test election during joint consensus
- Test edge case: overlapping configs (A,B,C → B,C,D)

---

#### **Task 4.3: Two-Phase Configuration Change** (6 hours)
**Goal:** Implement C_old,new → C_new transition.

**Implementation:**
```rust
// core/src/raft_node.rs
pub fn change_membership(
    &mut self,
    new_members: C,
) -> Result<LogIndex, ConfigChangeError>
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    if self.role != NodeState::Leader {
        return Err(ConfigChangeError::NotLeader);
    }

    // Check not already in joint consensus
    if self.configuration_state.is_joint() {
        return Err(ConfigChangeError::ChangeInProgress);
    }

    if self.pending_config_index.is_some() {
        return Err(ConfigChangeError::ChangeInProgress);
    }

    // Get current stable config
    let old_config = match &self.configuration_state {
        ConfigurationState::Stable(c) => c.clone(),
        _ => unreachable!(), // Already checked
    };

    let new_config = Configuration::new(new_members);

    // Create C_old,new entry
    let old_vec: Vec<NodeId> = old_config.members.iter().collect();
    let new_vec: Vec<NodeId> = new_config.members.iter().collect();

    let entry = LogEntry {
        term: self.current_term,
        entry_type: EntryType::JointConfig {
            old: old_vec.clone(),
            new: new_vec.clone(),
        },
    };

    self.storage.append_entries(&[entry]);
    let joint_index = self.storage.last_log_index();

    // Switch to joint consensus IMMEDIATELY
    self.configuration_state = ConfigurationState::Joint {
        old: old_config,
        new: new_config,
    };

    self.pending_config_index = Some(joint_index);

    // Initialize replication for any NEW members
    self.initialize_new_members_for_joint();

    // Replicate to ALL members (old + new)
    self.send_append_entries_to_followers();

    Ok(joint_index)
}

fn initialize_new_members_for_joint(&mut self) {
    if let ConfigurationState::Joint { old, new } = &self.configuration_state {
        for member in new.members.iter() {
            if !old.members.iter().any(|id| id == member) {
                // New member not in old config
                self.replication.initialize_peer(member, &self.storage);
                self.replication.mark_server_catching_up(member);

                if self.storage.first_log_index() > 1 {
                    self.send_install_snapshot_to(member);
                }
            }
        }
    }
}

// When C_old,new commits, append C_new
fn on_commit_advanced(&mut self, new_commit: LogIndex)
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    if let Some(pending_idx) = self.pending_config_index {
        if new_commit >= pending_idx {
            // Check if this was a C_old,new commit
            if self.configuration_state.is_joint() {
                // Now append C_new to finalize
                self.finalize_joint_consensus();
            } else {
                // Was a C_new commit, clear pending
                self.pending_config_index = None;
            }
        }
    }
}

fn finalize_joint_consensus(&mut self)
where
    SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
{
    if let ConfigurationState::Joint { new, .. } = &self.configuration_state {
        let new_vec: Vec<NodeId> = new.members.iter().collect();

        let entry = LogEntry {
            term: self.current_term,
            entry_type: EntryType::NewConfig(new_vec),
        };

        self.storage.append_entries(&[entry]);

        // C_new will commit quickly since we're in joint consensus
        self.send_append_entries_to_followers();

        // Note: Don't transition to Stable yet - wait for C_new to commit
        // That happens in apply_log_entry when C_new is applied
    }
}

// Applying entries
fn apply_log_entry(&mut self, index: LogIndex, entry: &LogEntry<P>) {
    match &entry.entry_type {
        EntryType::Command(payload) => {
            self.state_machine.apply(payload);
        }
        EntryType::ConfigChange(change) => {
            // Single-server change (backward compat)
            self.apply_single_server_change(change.clone());
        }
        EntryType::JointConfig { old, new } => {
            // Transition to joint consensus
            let old_config = self.vec_to_config(old);
            let new_config = self.vec_to_config(new);

            self.configuration_state = ConfigurationState::Joint {
                old: old_config,
                new: new_config,
            };

            self.initialize_new_members_for_joint();
        }
        EntryType::NewConfig(members) => {
            // Finalize to stable config
            let config = self.vec_to_config(members);
            self.configuration_state = ConfigurationState::Stable(config);
            self.pending_config_index = None;

            // Clean up removed members
            self.cleanup_removed_members();

            // Step down if we were removed
            if !self.configuration_state.is_member(self.id) {
                self.step_down(self.current_term);
            }
        }
    }
}
```

**Changes Required:**
- Add `change_membership()` API (replaces add/remove for multi-server changes)
- Keep `add_server()`/`remove_server()` for backward compat (delegate to single-change)
- Implement automatic C_new append when C_old,new commits
- Handle three entry types in apply
- Initialize new members during joint consensus

**Testing:**
- Test basic joint consensus (replace 1 node)
- Test multi-server change (replace 2 nodes)
- Test automatic C_new appending
- Test transition Stable → Joint → Stable

---

#### **Task 4.4: Snapshot Joint State** (2 hours)
**Goal:** Preserve joint consensus state in snapshots.

**Implementation:**
```rust
// core/src/snapshot.rs
#[derive(Clone, Debug, PartialEq)]
pub enum SnapshotConfiguration {
    Stable { members: Vec<NodeId> },
    Joint {
        old: Vec<NodeId>,
        new: Vec<NodeId>,
    },
}

pub struct SnapshotMetadata {
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
    pub configuration: SnapshotConfiguration,
}

// In create_snapshot_internal
let snapshot_config = match &self.configuration_state {
    ConfigurationState::Stable(c) => {
        SnapshotConfiguration::Stable {
            members: c.members.iter().collect(),
        }
    }
    ConfigurationState::Joint { old, new } => {
        SnapshotConfiguration::Joint {
            old: old.members.iter().collect(),
            new: new.members.iter().collect(),
        }
    }
};

// In constructor restoration
let configuration_state = if let Some(snapshot) = storage.load_snapshot() {
    match snapshot.metadata.configuration {
        SnapshotConfiguration::Stable { members } => {
            let config = self.vec_to_config(&members);
            ConfigurationState::Stable(config)
        }
        SnapshotConfiguration::Joint { old, new } => {
            let old_config = self.vec_to_config(&old);
            let new_config = self.vec_to_config(&new);
            ConfigurationState::Joint {
                old: old_config,
                new: new_config,
            }
        }
    }
} else {
    // ... default initialization
};
```

**Changes Required:**
- Extend `SnapshotConfiguration` enum
- Save joint state in snapshots
- Restore joint state on load
- Update serialization

**Testing:**
- Test snapshot during joint consensus
- Test restore from joint consensus snapshot
- Verify node continues joint consensus after restart

**End of Week 4 Checkpoint:**
- ✅ Joint Consensus fully implemented
- ✅ Double quorum working
- ✅ Two-phase protocol operational
- ⚠️ Need comprehensive testing

---

### Week 5: Testing & Validation

#### **Task 5.1: Joint Consensus Core Tests** (6 hours)

**Tests to Implement:**

1. **test_joint_consensus_double_quorum_commit** (1.5h)
   - 3-node cluster (1,2,3) → 4-node cluster (1,2,3,4)
   - Leader submits command during C_old,new
   - Verify: Needs majority in {1,2,3} AND {1,2,3,4}
   - Partition (1,2) from (3,4), verify no commit (missing old majority)
   - Heal, verify commit succeeds

2. **test_joint_consensus_double_quorum_election** (1.5h)
   - 5-node cluster (1,2,3,4,5) → (2,3,4,5,6)
   - Trigger election during joint consensus
   - Verify: Candidate needs votes from majority of BOTH configs
   - Test partial vote (majority in new, not old) → no election win

3. **test_replace_majority** (1h)
   - 3-node (A,B,C) → (X,Y,Z) complete replacement
   - Verify: Joint consensus (A,B,C) + (X,Y,Z) works
   - Verify: Commits require 2 from old + 2 from new
   - Verify: Finalizes to (X,Y,Z)

4. **test_leader_removed_in_joint_consensus** (1h)
   - 3-node cluster, node 1 is leader
   - Change to (2,3,4) - removing leader
   - Verify: Leader stays during C_old,new (in old config)
   - Verify: Leader steps down when C_new commits

5. **test_partition_during_joint_consensus** (1h)
   - 5-node (1,2,3,4,5) → (1,2,3,6,7)
   - Start change, partition (1,2,6,7) from (3,4,5)
   - Verify: C_old,new fails to commit (missing old majority)
   - Heal, verify change completes

6. **test_crash_during_joint_consensus** (1h)
   - Leader starts change, crashes after C_old,new appended but before C_new
   - New leader elected
   - Verify: New leader completes transition (appends C_new)

---

#### **Task 5.2: Edge Case Tests** (4 hours)

7. **test_concurrent_joint_consensus_rejected** (30min)
   - Start change to config A
   - Before complete, try to start change to config B
   - Verify: Second change rejected

8. **test_backward_compat_single_server_changes** (1h)
   - Use old `add_server()` API
   - Verify: Still works, doesn't use joint consensus
   - Mix old and new APIs

9. **test_snapshot_during_joint_consensus** (1h)
   - Start membership change
   - Trigger snapshot while in C_old,new
   - Restart node from snapshot
   - Verify: Continues joint consensus correctly

10. **test_multiple_sequential_changes** (1h)
    - Change (1,2,3) → (1,2,4)
    - Wait for completion
    - Change (1,2,4) → (1,5,6)
    - Verify: Each completes before next starts

11. **test_new_servers_catch_up_during_joint** (1h)
    - 3-node cluster with 100 entries
    - Change to (1,2,3,4,5) adding 2 nodes
    - Verify: New nodes catch up via InstallSnapshot
    - Verify: Commits continue (don't wait for full catch-up)

---

#### **Task 5.3: Chaos Testing** (2 hours)

12. **test_chaos_membership_changes** (2h)
    - Implement long-running test (1000+ operations)
    - Random operations: add server, remove server, client commands
    - Random failures: partitions, crashes, message drops
    - Invariants:
      - Single leader per term
      - Committed entries never lost
      - State machine converges across all nodes
      - Configuration changes eventually complete

**Implementation:**
```rust
#[test]
fn test_chaos_membership_changes() {
    let mut cluster = TimelessTestCluster::with_nodes(3);
    let mut rng = StdRng::seed_from_u64(42);

    for _ in 0..1000 {
        match rng.gen_range(0..10) {
            0..=1 => {
                // Add server (20%)
                if cluster.node_count() < 7 {
                    let new_id = cluster.max_node_id() + 1;
                    cluster.add_node(new_id);
                    // Leader adds server
                    // ...
                }
            }
            2..=3 => {
                // Remove server (20%)
                if cluster.node_count() > 3 {
                    let to_remove = cluster.random_non_leader_node(&mut rng);
                    // Leader removes server
                    // ...
                }
            }
            4..=7 => {
                // Client command (40%)
                cluster.submit_random_command(&mut rng);
            }
            8 => {
                // Partition (10%)
                cluster.random_partition(&mut rng);
            }
            9 => {
                // Heal partition (10%)
                cluster.heal_partition();
            }
            _ => unreachable!(),
        }

        cluster.deliver_messages();

        // Check invariants periodically
        if i % 100 == 0 {
            cluster.verify_single_leader();
            cluster.verify_log_matching();
            cluster.verify_state_machine_consistency();
        }
    }

    // Final verification
    cluster.heal_partition();
    cluster.wait_for_quiescence();
    cluster.verify_all_invariants();
}
```

---

#### **Task 5.4: Documentation & Examples** (2 hours)

**Documentation to write:**

1. **API Usage Guide** (`docs/dynamic_membership_usage.md`)
   - When to use `add_server()` vs `change_membership()`
   - How to safely remove nodes
   - Best practices for operational changes
   - Error handling

2. **Safety Properties** (`docs/dynamic_membership_safety.md`)
   - Explanation of joint consensus
   - Why double quorum is necessary
   - Proof sketch of safety
   - Edge cases and limitations

3. **Code Examples** (`raft/examples/membership_change.rs`)
   - Complete example of adding a server
   - Example of replacing multiple servers
   - Error handling patterns

**End of Week 5 Checkpoint:**
- ✅ Joint Consensus fully tested
- ✅ Chaos testing validates correctness
- ✅ Documentation complete
- ✅ Production-ready for arbitrary membership changes
- **🎉 Milestone: Joint Consensus complete**

---

## Summary Timeline

| Week | Phase | Focus | Deliverable |
|------|-------|-------|-------------|
| 1 | A | Foundation | Dynamic quorum infrastructure |
| 2 | A | Protocol | Configuration change working end-to-end |
| 3 | A | Hardening | Single-Server production-ready ✅ |
| 4 | B | Upgrade | Joint Consensus implemented |
| 5 | B | Testing | Joint Consensus production-ready ✅ |

**Total Effort:** 50-65 hours over 5 weeks

---

## Risk Mitigation

### **High-Risk Areas**
1. **Quorum calculation bugs** - Can cause split-brain
   - Mitigation: Extensive unit tests, chaos testing
   - Review: Double-check median calculation logic

2. **Timing of config application** - Apply before commit vs after
    - Mitigation: Apply to state only on commit; update active quorum config when entries are appended/received
   - Testing: Verify with partition scenarios

3. **Snapshot edge cases** - Config must survive compaction
   - Mitigation: Test snapshot creation/restoration thoroughly
   - Verify: Config restoration after crash

### **Testing Strategy**
- **Unit tests:** Each component (quorum, election, apply)
- **Integration tests:** Full end-to-end scenarios
- **Edge case tests:** Partitions, crashes, concurrent changes
- **Chaos tests:** Randomized long-running validation

### **Rollout Strategy**
1. Deploy Single-Server to staging (Week 3)
2. Stress test in production-like environment
3. Deploy to production with monitoring
4. After 2 weeks stability, begin Joint Consensus work
5. Deploy Joint Consensus as opt-in feature flag
6. Gradually migrate to Joint Consensus API

---

## Success Criteria

### **Phase A: Single-Server (Week 3)**
- ✅ Can add one server to running cluster
- ✅ Can remove one server from running cluster
- ✅ Configuration survives snapshots and restarts
- ✅ All existing Raft tests still passing
- ✅ 10+ new membership tests passing
- ✅ Manual testing shows stable operation

### **Phase B: Joint Consensus (Week 5)**
- ✅ Can add/remove multiple servers simultaneously
- ✅ Double quorum working correctly
- ✅ Two-phase protocol (C_old,new → C_new) operational
- ✅ Handles partitions during membership changes safely
- ✅ 20+ total membership tests passing
- ✅ Chaos test runs 10,000+ operations without invariant violations
- ✅ Documentation complete and accurate

---

## References

- **Raft Paper:** Ongaro & Ousterhout (2014) - Section 6
- **Raft Thesis:** Ongaro (2014) - Chapter 4 (detailed membership algorithms)
- **Single-Server Changes:** Thesis Section 4.3
- **Joint Consensus:** Thesis Section 4.1-4.2

---

**Next Steps:**
1. Review this plan with team
2. Create feature branch: `git checkout -b feature/dynamic-membership`
3. Start with Week 1, Task 1.1
4. Commit frequently with descriptive messages
5. Run full test suite after each task
6. Update this document with actual effort vs estimates
