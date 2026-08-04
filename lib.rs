//! # Multi-Agency Protocol (MAP)
//!
//! Core library pour la coordination inter-agences sur Mars.
//! Implémente les types de messages, l'authentification et la négociation
//! de ressources entre entités de différentes agences spatiales.
//!
//! ## Architecture
//!
//! MAP fonctionne comme un protocole de couche applicative au-dessus de DTN++.
//! Chaque entité (rover, habitat, orbiteur) possède un identifiant unique
//! et un ensemble de capacités qu'elle peut offrir ou consommer.
//!
//! ## Exemple d'utilisation
//!
//! ```rust,no_run
//! use multi_agency_protocol::{MapNode, MapConfig, AgencyId};
//!
//! let config = MapConfig {
//!     node_id: "rover-nasa-01".into(),
//!     agency: AgencyId::Nasa,
//!     capabilities: vec!["mobility".into(), "sampling".into()],
//!     priority: 10,
//! };
//!
//! let node = MapNode::new(config);
//! ```

pub mod message_types;
pub mod auth;
pub mod negotiation;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use tracing::{info, warn, error};

// ─── Identifiants et Types de Base ───

/// Identifiant unique d'une agence spatiale
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgencyId {
    Nasa,
    Spacex,
    Esa,
    Cnsa,
    Roscosmos,
    Isro,
    Other(String),
}

impl std::fmt::Display for AgencyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgencyId::Nasa => write!(f, "NASA"),
            AgencyId::Spacex => write!(f, "SPACEX"),
            AgencyId::Esa => write!(f, "ESA"),
            AgencyId::Cnsa => write!(f, "CNSA"),
            AgencyId::Roscosmos => write!(f, "ROSCOSMOS"),
            AgencyId::Isro => write!(f, "ISRO"),
            AgencyId::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Identifiant unique d'un noeud sur Mars
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type de plateforme martienne
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformType {
    WheeledRover,
    LeggedRover,
    AerialDrone,
    Habitat,
    Orbiter,
    Relay,
}

/// Statut d'un noeud MAP
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Standby,
    Degraded,
    SafeMode,
    Offline,
}

// ─── Configuration ───

/// Configuration d'un noeud MAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    pub node_id: String,
    pub agency: AgencyId,
    pub capabilities: Vec<String>,
    pub priority: u8,
    pub platform: PlatformType,
}

// ─── Erreurs ───

#[derive(Error, Debug)]
pub enum MapError {
    #[error("Noeud non trouvé: {0}")]
    NodeNotFound(NodeId),

    #[error("Conflit de ressources: {0}")]
    ResourceConflict(String),

    #[error("Authentification échouée: {0}")]
    AuthFailed(String),

    #[error("Timeout de négociation: {0}ms")]
    NegotiationTimeout(u64),

    #[error("Capacité non supportée: {0}")]
    UnsupportedCapability(String),

    #[error("Priorité insuffisante: required={required}, provided={provided}")]
    InsufficientPriority { required: u8, provided: u8 },

    #[error("Erreur de sérialisation: {0}")]
    Serialization(String),
}

pub type MapResult<T> = Result<T, MapError>;

// ─── État du Noeud ───

/// État interne d'un noeud MAP
#[derive(Debug, Clone)]
pub struct NodeState {
    pub id: NodeId,
    pub agency: AgencyId,
    pub platform: PlatformType,
    pub status: NodeStatus,
    pub capabilities: Vec<String>,
    pub priority: u8,
    pub active_negotiations: usize,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

// ─── Noeud MAP Principal ───

/// Le noeud MAP principal qui gère la coordination locale
pub struct MapNode {
    pub state: Arc<RwLock<NodeState>>,
    pub config: MapConfig,
    pub peers: Arc<RwLock<HashMap<NodeId, PeerInfo>>>,
    pub message_log: Arc<RwLock<Vec<message_types::MapMessage>>>,
}

/// Informations sur un pair MAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: NodeId,
    pub agency: AgencyId,
    pub platform: PlatformType,
    pub capabilities: Vec<String>,
    pub status: NodeStatus,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub priority: u8,
}

impl MapNode {
    /// Crée un nouveau noeud MAP avec la configuration donnée
    pub fn new(config: MapConfig) -> Self {
        info!(
            "Initializing MAP node: {} ({})",
            config.node_id, config.agency
        );

        let state = NodeState {
            id: NodeId(config.node_id.clone()),
            agency: config.agency.clone(),
            platform: config.platform.clone(),
            status: NodeStatus::Active,
            capabilities: config.capabilities.clone(),
            priority: config.priority,
            active_negotiations: 0,
            last_heartbeat: chrono::Utc::now(),
        };

        Self {
            state: Arc::new(RwLock::new(state)),
            config,
            peers: Arc::new(RwLock::new(HashMap::new())),
            message_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Enregistre un pair découvert sur le réseau
    pub async fn register_peer(&self, peer: PeerInfo) -> MapResult<()> {
        let mut peers = self.peers.write().await;
        info!(
            "Registering peer: {} (agency={}, platform={:?})",
            peer.id, peer.agency, peer.platform
        );
        peers.insert(peer.id.clone(), peer);
        Ok(())
    }

    /// Supprime un pair de la liste
    pub async fn remove_peer(&self, node_id: &NodeId) -> MapResult<()> {
        let mut peers = self.peers.write().await;
        peers
            .remove(node_id)
            .ok_or_else(|| MapError::NodeNotFound(node_id.clone()))?;
        info!("Removed peer: {}", node_id);
        Ok(())
    }

    /// Récupère la liste des pairs actifs
    pub async fn active_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.status == NodeStatus::Active)
            .cloned()
            .collect()
    }

    /// Trouve les pairs offrant une capacité spécifique
    pub async fn find_by_capability(&self, capability: &str) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.capabilities.contains(&capability.to_string()))
            .cloned()
            .collect()
    }

    /// Met à jour le statut du noeud
    pub async fn set_status(&self, status: NodeStatus) {
        let mut state = self.state.write().await;
        warn!("Node {} status changed to {:?}", state.id, status);
        state.status = status;
    }

    /// Met à jour le heartbeat
    pub async fn heartbeat(&self) {
        let mut state = self.state.write().await;
        state.last_heartbeat = chrono::Utc::now();
    }

    /// Vérifie l'état de santé du noeud
    pub async fn health_check(&self) -> NodeState {
        self.state.read().await.clone()
    }

    /// Comptabilise une négociation active
    pub async fn increment_negotiations(&self) {
        let mut state = self.state.write().await;
        state.active_negotiations += 1;
    }

    /// Décrémente une négociation terminée
    pub async fn decrement_negotiations(&self) {
        let mut state = self.state.write().await;
        state.active_negotiations = state.active_negotiations.saturating_sub(1);
    }

    /// Génère un résumé de l'état du réseau
    pub async fn network_summary(&self) -> NetworkSummary {
        let state = self.state.read().await;
        let peers = self.peers.read().await;

        NetworkSummary {
            local_node: state.id.clone(),
            local_agency: state.agency.clone(),
            local_status: state.status.clone(),
            total_peers: peers.len(),
            active_peers: peers.values().filter(|p| p.status == NodeStatus::Active).count(),
            agencies_present: peers.values().map(|p| p.agency.clone()).collect::<std::collections::HashSet<_>>(),
            active_negotiations: state.active_negotiations,
        }
    }
}

/// Résumé de l'état du réseau MAP
#[derive(Debug, Clone)]
pub struct NetworkSummary {
    pub local_node: NodeId,
    pub local_agency: AgencyId,
    pub local_status: NodeStatus,
    pub total_peers: usize,
    pub active_peers: usize,
    pub agencies_present: std::collections::HashSet<AgencyId>,
    pub active_negotiations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_map_node() {
        let config = MapConfig {
            node_id: "rover-nasa-01".into(),
            agency: AgencyId::Nasa,
            capabilities: vec!["mobility".into(), "sampling".into()],
            priority: 10,
            platform: PlatformType::WheeledRover,
        };

        let node = MapNode::new(config);
        assert_eq!(node.state.blocking_read().id.0, "rover-nasa-01");
        assert_eq!(node.state.blocking_read().priority, 10);
    }

    #[tokio::test]
    async fn test_register_and_find_peer() {
        let config = MapConfig {
            node_id: "rover-nasa-01".into(),
            agency: AgencyId::Nasa,
            capabilities: vec!["mobility".into()],
            priority: 10,
            platform: PlatformType::WheeledRover,
        };

        let node = MapNode::new(config);

        let peer = PeerInfo {
            id: NodeId("habitat-esa-01".into()),
            agency: AgencyId::Esa,
            platform: PlatformType::Habitat,
            capabilities: vec!["power".into(), "comms".into()],
            status: NodeStatus::Active,
            last_seen: chrono::Utc::now(),
            priority: 5,
        };

        node.register_peer(peer).await.unwrap();

        let found = node.find_by_capability("power").await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id.0, "habitat-esa-01");
    }

    #[tokio::test]
    async fn test_network_summary() {
        let config = MapConfig {
            node_id: "rover-nasa-01".into(),
            agency: AgencyId::Nasa,
            capabilities: vec!["mobility".into()],
            priority: 10,
            platform: PlatformType::WheeledRover,
        };

        let node = MapNode::new(config);

        let peer1 = PeerInfo {
            id: NodeId("habitat-esa-01".into()),
            agency: AgencyId::Esa,
            platform: PlatformType::Habitat,
            capabilities: vec!["power".into()],
            status: NodeStatus::Active,
            last_seen: chrono::Utc::now(),
            priority: 5,
        };

        let peer2 = PeerInfo {
            id: NodeId("orbiter-spacex-01".into()),
            agency: AgencyId::Spacex,
            platform: PlatformType::Orbiter,
            capabilities: vec!["relay".into()],
            status: NodeStatus::Active,
            last_seen: chrono::Utc::now(),
            priority: 8,
        };

        node.register_peer(peer1).await.unwrap();
        node.register_peer(peer2).await.unwrap();

        let summary = node.network_summary().await;
        assert_eq!(summary.total_peers, 2);
        assert_eq!(summary.active_peers, 2);
        assert!(summary.agencies_present.contains(&AgencyId::Esa));
        assert!(summary.agencies_present.contains(&AgencyId::Spacex));
    }
}
