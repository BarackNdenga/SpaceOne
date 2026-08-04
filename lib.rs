//! # DTN++ — Delay-Tolerant Networking Avancé
//!
//! Extension du standard Bundle Protocol (RFC 9171) pour les communications
//! interplanétaires avec latence élevée (6-22 minutes Terre-Mars).
//!
//! ## Caractéristiques
//!
//! - **Priority Bundles** : Classification en 8 niveaux de priorité
//! - **AI Compression** : Compression adaptative par réseau neuronal embarqué
//! - **Store-and-Forward** : Buffering intelligent pendant les blackouts
//! - **Contact Plan** : Planification des fenêtres de communication

pub mod priority_bundle;
pub mod ai_compression;
pub mod store_forward;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

// ─── Types de Base ───

/// Identifiant unique d'un bundle DTN
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BundleId(pub String);

impl std::fmt::Display for BundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bundle:{}", self.0)
    }
}

/// Destination d'un bundle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BundleDestination {
    /// Noeud spécifique sur Mars
    MarsNode(String),
    /// Station au sol sur Terre
    EarthStation(String),
    /// Diffusion à tous les noeuds du mesh
    Broadcast,
    /// Noeud relais (orbiteur)
    Relay(String),
}

/// Statut de livraison d'un bundle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Queued,
    Transmitting,
    InTransit,
    Delivered,
    Failed { reason: String },
    Expired,
}

/// Fenêtre de contact (période de communication possible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPlan {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub partner: BundleDestination,
    pub bandwidth_kbps: f64,
    pub latency_minutes: f64,
    pub reliability: f64, // 0.0-1.0
}

// ─── Erreurs ───

#[derive(Error, Debug)]
pub enum DtnError {
    #[error("Buffer plein: capacity={capacity}")]
    BufferFull { capacity: usize },

    #[error("Aucune route vers: {0}")]
    NoRoute(BundleDestination),

    #[error("Bundle expiré: {0}")]
    BundleExpired(BundleId),

    #[error("Contact window fermée pour: {0}")]
    ContactWindowClosed(BundleDestination),

    #[error("Compression échouée: {0}")]
    CompressionFailed(String),
}

pub type DtnResult<T> = Result<T, DtnError>;

// ─── DTN Node ───

/// Noeud DTN principal
pub struct DtnNode {
    /// Identifiant de ce noeud
    pub local_id: String,
    /// Buffer de stockage (store-and-forward)
    pub buffer: Arc<RwLock<VecDeque<priority_bundle::PriorityBundle>>>,
    /// Capacité maximale du buffer (bundles)
    pub buffer_capacity: usize,
    /// Plan de contacts actifs
    pub contact_plans: Arc<RwLock<Vec<ContactPlan>>>,
    /// Statistiques de transmission
    pub stats: Arc<RwLock<DtnStats>>,
}

/// Statistiques DTN
#[derive(Debug, Clone, Default, Serialize)]
pub struct DtnStats {
    pub bundles_sent: u64,
    pub bundles_received: u64,
    pub bundles_delivered: u64,
    pub bundles_failed: u64,
    pub bytes_transmitted: u64,
    pub bytes_buffered: u64,
    pub current_buffer_count: usize,
    pub average_latency_minutes: f64,
    pub contact_windows_used: u64,
}

impl DtnNode {
    /// Crée un nouveau noeud DTN
    pub fn new(local_id: String, buffer_capacity: usize) -> Self {
        info!("DTN++ node initialized: {} (buffer={})", local_id, buffer_capacity);
        Self {
            local_id,
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(buffer_capacity))),
            buffer_capacity,
            contact_plans: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(DtnStats::default())),
        }
    }

    /// Enqueue un bundle pour transmission
    pub async fn enqueue(&self, bundle: priority_bundle::PriorityBundle) -> DtnResult<()> {
        let mut buffer = self.buffer.write().await;
        if buffer.len() >= self.buffer_capacity {
            return Err(DtnError::BufferFull { capacity: self.buffer_capacity });
        }
        buffer.push_back(bundle);
        info!("Bundle enqueued, buffer size: {}", buffer.len());
        Ok(())
    }

    /// Déqueue le prochain bundle par priorité
    pub async fn dequeue_highest_priority(&self) -> Option<priority_bundle::PriorityBundle> {
        let mut buffer = self.buffer.write().await;
        if buffer.is_empty() {
            return None;
        }

        // Trouver l'index du bundle de plus haute priorité
        let mut best_idx = 0;
        let mut best_priority = buffer[0].priority;
        for (i, bundle) in buffer.iter().enumerate() {
            if bundle.priority > best_priority {
                best_priority = bundle.priority;
                best_idx = i;
            }
        }

        buffer.remove(best_idx)
    }

    /// Enregistre un plan de contact
    pub async fn add_contact_plan(&self, plan: ContactPlan) {
        let mut plans = self.contact_plans.write().await;
        plans.push(plan);
        info!("Contact plan added, total: {}", plans.len());
    }

    /// Vérifie si une fenêtre de contact est disponible
    pub async fn has_active_contact(&self, partner: &BundleDestination) -> Option<&ContactPlan> {
        let plans = self.contact_plans.read().await;
        let now = Utc::now();
        plans
            .iter()
            .find(|p| &p.partner == partner && p.start <= now && p.end >= now)
    }

    /// Statistiques courantes
    pub async fn get_stats(&self) -> DtnStats {
        let stats = self.stats.read().await;
        let buffer = self.buffer.read().await;
        let mut s = stats.clone();
        s.current_buffer_count = buffer.len();
        s.bytes_buffered = buffer.iter().map(|b| b.payload_size).sum();
        s
    }

    /// Nettoie les bundles expirés
    pub async fn cleanup_expired(&self) -> usize {
        let mut buffer = self.buffer.write().await;
        let now = Utc::now();
        let initial = buffer.len();
        buffer.retain(|b| b.expires_at > now);
        let removed = initial - buffer.len();
        if removed > 0 {
            warn!("Cleaned up {} expired bundles", removed);
        }
        removed
    }

    /// Taille actuelle du buffer
    pub async fn buffer_size(&self) -> usize {
        self.buffer.read().await.len()
    }

    /// Capacité restante
    pub async fn remaining_capacity(&self) -> usize {
        let buffer = self.buffer.read().await;
        self.buffer_capacity.saturating_sub(buffer.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use priority_bundle::{PriorityBundle, BundlePriority};

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let node = DtnNode::new("rover-01".into(), 100);

        let bundle1 = PriorityBundle {
            id: BundleId("b1".into()),
            priority: BundlePriority::Normal,
            source: "rover-01".into(),
            destination: BundleDestination::Relay("orbiter-01".into()),
            payload: vec![1, 2, 3],
            payload_size: 3,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            ttl: 86400,
        };

        node.enqueue(bundle1.clone()).await.unwrap();
        assert_eq!(node.buffer_size().await, 1);

        let dequeued = node.dequeue_highest_priority().await;
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().id, BundleId("b1".into()));
    }

    #[tokio::test]
    async fn test_buffer_capacity() {
        let node = DtnNode::new("test".into(), 2);

        let bundle = || PriorityBundle {
            id: BundleId(uuid::Uuid::new_v4().to_string()),
            priority: BundlePriority::Normal,
            source: "test".into(),
            destination: BundleDestination::EarthStation("deep-space-network".into()),
            payload: vec![0],
            payload_size: 1,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            ttl: 3600,
        };

        node.enqueue(bundle()).await.unwrap();
        node.enqueue(bundle()).await.unwrap();

        let result = node.enqueue(bundle()).await;
        assert!(result.is_err()); // Buffer plein
    }

    #[tokio::test]
    async fn test_contact_plan() {
        let node = DtnNode::new("orbiter-01".into(), 100);

        let plan = ContactPlan {
            start: Utc::now() - chrono::Duration::minutes(30),
            end: Utc::now() + chrono::Duration::hours(2),
            partner: BundleDestination::EarthStation("dsn-madrid".into()),
            bandwidth_kbps: 2.0,
            latency_minutes: 11.0,
            reliability: 0.95,
        };

        node.add_contact_plan(plan).await;

        let contact = node.has_active_contact(&BundleDestination::EarthStation("dsn-madrid".into())).await;
        assert!(contact.is_some());
        assert!((contact.unwrap().bandwidth_kbps - 2.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_stats() {
        let node = DtnNode::new("stats-test".into(), 50);
        let stats = node.get_stats().await;
        assert_eq!(stats.current_buffer_count, 0);
        assert_eq!(stats.bundles_sent, 0);
    }
}
