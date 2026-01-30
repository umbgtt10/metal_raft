// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    collections::{
        config_change_collection::ConfigChangeCollection, configuration::Configuration,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    components::{
        config_change_manager::{ConfigChangeManager, ConfigError},
        log_replication_manager::LogReplicationManager,
    },
    log_entry::ConfigurationChange,
    node_state::NodeState,
};
use raft_test_utils::{
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection, null_observer::NullObserver,
};

fn make_test_config(nodes: &[u64]) -> Configuration<InMemoryNodeCollection> {
    let mut members = InMemoryNodeCollection::new();

    members.push(1).unwrap();
    for &node_id in nodes {
        members.push(node_id).unwrap();
    }
    Configuration::new(members)
}

#[test]
fn test_new_manager() {
    // Arrange
    let config = make_test_config(&[2, 3]);

    // Act
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Assert
    assert_eq!(manager.config().size(), 3);
    assert!(manager.config().contains(1));
    assert!(manager.config().contains(2));
    assert!(manager.config().contains(3));
}

#[test]
fn test_config_getter() {
    // Arrange
    let config = make_test_config(&[2, 3, 4]);
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let retrieved_config = manager.config();

    // Assert
    assert_eq!(retrieved_config.size(), 4);
    assert_eq!(retrieved_config.quorum_size(), 3);
}

#[test]
fn test_add_server_not_leader() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.add_server(4, 1, false, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_add_server_already_exists_in_peers() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.add_server(2, 1, true, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_self_already_exists() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.add_server(1, 1, true, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_config_change_in_progress() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // Act
    let result2 = manager.add_server(5, 1, true, 5);

    // Assert
    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_add_server_allows_new_change_after_commit() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // Act
    let result2 = manager.add_server(5, 1, true, 10);

    // Assert
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), ConfigurationChange::AddServer(5));
}

#[test]
fn test_add_server_returns_correct_change() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.add_server(4, 1, true, 0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::AddServer(4));
}

#[test]
fn test_remove_server_not_leader() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(3, 1, false, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_remove_server_not_found() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(99, 1, true, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::NodeNotFound));
}

#[test]
fn test_remove_server_last_node() {
    // Arrange
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(1, 1, true, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::CannotRemoveLastNode));
}

#[test]
fn test_remove_server_self_allowed() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(1, 1, true, 0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(1));
}

#[test]
fn test_remove_server_config_change_in_progress() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result1 = manager.remove_server(3, 1, true, 0);

    // Assert
    assert!(result1.is_ok());

    // Act
    manager.track_pending_change(10);
    let result2 = manager.remove_server(2, 1, true, 5);

    // Assert
    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_remove_server_returns_correct_change() {
    // Arrange
    let config = make_test_config(&[2, 3, 4]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(3, 1, true, 0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(3));
}

#[test]
fn test_track_pending_change() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);
    manager.track_pending_change(42);

    // Act
    let result = manager.add_server(4, 1, true, 10);

    // Assert
    assert_eq!(result, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_is_change_committed() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act & Assert
    assert!(manager.is_change_committed(5, 10));
    assert!(manager.is_change_committed(10, 10));
    assert!(!manager.is_change_committed(15, 10));
}

#[test]
fn test_pending_change_cleared_after_commit() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result1 = manager.add_server(4, 1, true, 0);

    // Assert
    assert!(result1.is_ok());

    // Act
    manager.track_pending_change(10);
    let result2 = manager.add_server(5, 1, true, 9);
    let result3 = manager.add_server(5, 1, true, 10);

    // Assert
    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
    assert!(result3.is_ok());
}

#[test]
fn test_add_to_empty_cluster() {
    // Arrange
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.add_server(2, 1, true, 0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::AddServer(2));
}

#[test]
fn test_remove_from_two_node_cluster() {
    // Arrange
    let config = make_test_config(&[2]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(2, 1, true, 0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(2));
}

#[test]
fn test_cannot_remove_only_member() {
    // Arrange
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result = manager.remove_server(1, 1, true, 0);

    // Assert
    assert_eq!(result, Err(ConfigError::CannotRemoveLastNode));
}

#[test]
fn test_multiple_changes_in_sequence() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let result1 = manager.add_server(4, 1, true, 0);

    // Assert
    assert!(result1.is_ok());

    // Act
    manager.track_pending_change(10);
    let result2 = manager.remove_server(3, 1, true, 10);

    // Assert
    assert!(result2.is_ok());

    // Act
    manager.track_pending_change(15);
    let result3 = manager.add_server(5, 1, true, 14);
    let result4 = manager.add_server(5, 1, true, 15);

    // Assert
    assert_eq!(result3, Err(ConfigError::ConfigChangeInProgress));
    assert!(result4.is_ok());
}

#[test]
fn test_config_manager_workflow() {
    // Arrange
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Act
    let change = manager.add_server(4, 1, true, 0).unwrap();

    // Assert
    assert_eq!(change, ConfigurationChange::AddServer(4));

    // Act
    manager.track_pending_change(10);
    let result = manager.add_server(5, 1, true, 9);
    let change2 = manager.remove_server(2, 1, true, 10);

    // Assert
    assert_eq!(result, Err(ConfigError::ConfigChangeInProgress));
    assert!(change2.is_ok());
    assert_eq!(change2.unwrap(), ConfigurationChange::RemoveServer(2));
}

#[test]
fn test_apply_changes_add_server() {
    // Arrange
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    let config = Configuration::new(members);
    let mut manager =
        ConfigChangeManager::<InMemoryNodeCollection, InMemoryMapCollection>::new(config);
    let mut changes = InMemoryConfigChangeCollection::new();
    let _ = changes.push(1, ConfigurationChange::AddServer(2));
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let mut current_role = NodeState::Leader;
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Act
    manager.apply_changes(
        changes,
        1,
        5,
        &mut replication,
        &mut observer,
        &mut current_role,
    );

    // Assert
    assert!(manager.config().contains(2));
    assert_eq!(replication.next_index().get(2), Some(6));
    assert_eq!(replication.match_index().get(2), Some(0));
}

#[test]
fn test_apply_changes_remove_server() {
    // Arrange
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut manager =
        ConfigChangeManager::<InMemoryNodeCollection, InMemoryMapCollection>::new(config);
    let mut changes = InMemoryConfigChangeCollection::new();
    let _ = changes.push(1, ConfigurationChange::RemoveServer(2));
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let mut current_role = NodeState::Leader;
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Act
    manager.apply_changes(
        changes,
        1,
        5,
        &mut replication,
        &mut observer,
        &mut current_role,
    );

    // Assert
    assert!(!manager.config().contains(2));
    assert!(replication.next_index().get(2).is_none());
}

#[test]
fn test_apply_changes_remove_self() {
    // Arrange
    let mut members = InMemoryNodeCollection::new();
    members.push(1).unwrap();
    members.push(2).unwrap();
    let config = Configuration::new(members);
    let mut manager =
        ConfigChangeManager::<InMemoryNodeCollection, InMemoryMapCollection>::new(config);
    let mut changes = InMemoryConfigChangeCollection::new();
    let _ = changes.push(1, ConfigurationChange::RemoveServer(1));
    let mut observer = NullObserver::<String, InMemoryLogEntryCollection>::new();
    let mut current_role = NodeState::Leader;
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();

    // Act
    manager.apply_changes(
        changes,
        1,
        5,
        &mut replication,
        &mut observer,
        &mut current_role,
    );

    // Assert
    assert!(!manager.config().contains(1));
    assert_eq!(current_role, NodeState::Follower);
}
