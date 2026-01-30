// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    collections::{
        config_change_collection::ConfigChangeCollection, configuration::Configuration,
        map_collection::MapCollection, node_collection::NodeCollection,
    },
    components::log_replication_manager::LogReplicationManager,
    log_entry::ConfigurationChange,
    node_state::NodeState,
    observer::Observer,
    types::{LogIndex, NodeId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    NotLeader,
    ConfigChangeInProgress,
    NodeAlreadyExists,
    NodeNotFound,
    CannotRemoveLastNode,
}

pub struct ConfigChangeManager<C, M>
where
    C: NodeCollection,
{
    config: Configuration<C>,
    pending_change: Option<LogIndex>,
    _phantom: core::marker::PhantomData<M>,
}

impl<C, M> ConfigChangeManager<C, M>
where
    C: NodeCollection,
    M: MapCollection,
{
    pub fn new(config: Configuration<C>) -> Self {
        Self {
            config,
            pending_change: None,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn config(&self) -> &Configuration<C> {
        &self.config
    }

    pub fn is_change_committed(&self, index: LogIndex, commit_index: LogIndex) -> bool {
        index <= commit_index
    }

    pub fn add_server(
        &mut self,
        node_id: NodeId,
        self_id: NodeId,
        is_leader: bool,
        commit_index: LogIndex,
    ) -> Result<ConfigurationChange, ConfigError> {
        if !is_leader {
            return Err(ConfigError::NotLeader);
        }

        if let Some(pending_index) = self.pending_change {
            if !self.is_change_committed(pending_index, commit_index) {
                return Err(ConfigError::ConfigChangeInProgress);
            }
        }

        if node_id == self_id || self.config.contains(node_id) {
            return Err(ConfigError::NodeAlreadyExists);
        }

        Ok(ConfigurationChange::AddServer(node_id))
    }

    pub fn remove_server(
        &mut self,
        node_id: NodeId,
        self_id: NodeId,
        is_leader: bool,
        commit_index: LogIndex,
    ) -> Result<ConfigurationChange, ConfigError> {
        if !is_leader {
            return Err(ConfigError::NotLeader);
        }

        if let Some(pending_index) = self.pending_change {
            if !self.is_change_committed(pending_index, commit_index) {
                return Err(ConfigError::ConfigChangeInProgress);
            }
        }

        if self.config.size() == 1 {
            return Err(ConfigError::CannotRemoveLastNode);
        }

        if node_id != self_id && !self.config.contains(node_id) {
            return Err(ConfigError::NodeNotFound);
        }

        Ok(ConfigurationChange::RemoveServer(node_id))
    }

    pub fn track_pending_change(&mut self, index: LogIndex) {
        self.pending_change = Some(index);
    }

    pub fn apply_changes<CCC, O, P, L, CC>(
        &mut self,
        changes: CCC,
        self_id: NodeId,
        last_log_index: LogIndex,
        replication: &mut LogReplicationManager<M>,
        observer: &mut O,
        current_role: &mut NodeState,
    ) where
        CCC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        for (index, change) in changes.iter() {
            match change {
                ConfigurationChange::AddServer(node_id) => {
                    observer.configuration_change_applied(self_id, *node_id, true);

                    let mut new_members = C::new();
                    for existing_id in self.config.members.iter() {
                        let _ = new_members.push(existing_id);
                    }
                    let _ = new_members.push(*node_id);
                    self.config = Configuration::new(new_members);

                    if *current_role == NodeState::Leader {
                        let next_index = last_log_index + 1;
                        replication.next_index_mut().insert(*node_id, next_index);
                        replication.match_index_mut().insert(*node_id, 0);

                        replication.mark_server_catching_up(*node_id);
                    }
                }
                ConfigurationChange::RemoveServer(node_id) => {
                    observer.configuration_change_applied(self_id, *node_id, false);

                    let mut new_members = C::new();
                    for existing_id in self.config.members.iter() {
                        if existing_id != *node_id {
                            let _ = new_members.push(existing_id);
                        }
                    }
                    self.config = Configuration::new(new_members);

                    if *current_role == NodeState::Leader {
                        replication.next_index_mut().remove(*node_id);
                        replication.match_index_mut().remove(*node_id);
                    }

                    if *node_id == self_id {
                        *current_role = NodeState::Follower;
                    }
                }
            }

            if let Some(pending_index) = self.pending_change {
                if index == pending_index {
                    self.pending_change = None;
                }
            }
        }
    }
}
