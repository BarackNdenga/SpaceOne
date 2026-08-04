//! # Priority Bundles — Classification des Bundles DTN
//!
//! Chaque bundle est classé en un niveau de priorité qui détermine
//! l'ordre de transmission pendant les fenêtres de contact limitées.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::BundleDestination;

/// Niveaux de priorité (0 = le plus bas, 7 = le plus critique)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BundlePriority {
    /// Données de maintenance, logs différé
    Maintenance = 0,
    /// Données scientifiques non-critiques
    ScienceBulk = 1,
    /// Données scientifiques standard
    ScienceNormal = 2,
    /// Telemetry routine
    Telemetry = 3,
    /// Données scientifiques haute priorité
    ScienceHigh = 4,
    /// Commandes mission
    Command = 5,
    /// Données d'urgence
    Emergency = 6,
    /// Safety-critical (vie équipage, survie)
    SafetyCritical = 7,
}

impl BundlePriority {
    /// Temps maximum de retard acceptable pour cette priorité
    pub fn max_delay_seconds(&self) -> u32 {
        match self {
            BundlePriority::SafetyCritical => 30,      // 30 secondes
            BundlePriority::Emergency => 120,           // 2 minutes
            BundlePriority::Command => 300,             // 5 minutes
            BundlePriority::ScienceHigh => 3600,        // 1 heure
            BundlePriority::Telemetry => 7200,          // 2 heures
            BundlePriority::ScienceNormal => 43200,     // 12 heures
            BundlePriority::ScienceBulk => 86400,       // 24 heures
            BundlePriority::Maintenance => 259200,      // 3 jours
        }
    }

    /// Bandwidth minimum requis (kbps)
    pub fn min_bandwidth_kbps(&self) -> f64 {
        match self {
            BundlePriority::SafetyCritical => 2.0,
            BundlePriority::Emergency => 1.0,
            BundlePriority::Command => 0.5,
            _ => 0.1,
        }
    }
}

/// Bundle prioritaire DTN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityBundle {
    pub id: crate::BundleId,
    pub priority: BundlePriority,
    pub source: String,
    pub destination: BundleDestination,
    pub payload: Vec<u8>,
    pub payload_size: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ttl: u32, // Time-to-live en secondes
}

impl PriorityBundle {
    /// Crée un nouveau bundle
    pub fn new(
        id: crate::BundleId,
        priority: BundlePriority,
        source: String,
        destination: BundleDestination,
        payload: Vec<u8>,
        ttl_seconds: u32,
    ) -> Self {
        let payload_size = payload.len() as u64;
        Self {
            id,
            priority,
            source,
            destination,
            payload,
            payload_size,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(ttl_seconds as i64),
            ttl: ttl_seconds,
        }
    }

    /// Vérifie si le bundle est encore valide
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Temps restant avant expiration
    pub fn remaining_ttl_seconds(&self) -> i64 {
        self.expires_at.signed_duration_since(Utc::now()).num_seconds()
    }

    /// Vérifie si ce bundle doit être transmis en priorité
    pub fn is_urgent(&self) -> bool {
        self.remaining_ttl_seconds() < (self.priority.max_delay_seconds() as i64) / 2
    }

    /// Estime le temps de transmission à une bande passante donnée
    pub fn estimated_transmission_time_seconds(&self, bandwidth_kbps: f64) -> f64 {
        (self.payload_size as f64 * 8.0) / (bandwidth_kbps * 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(BundlePriority::SafetyCritical > BundlePriority::Emergency);
        assert!(BundlePriority::Emergency > BundlePriority::Command);
        assert!(BundlePriority::Command > BundlePriority::ScienceHigh);
        assert!(BundlePriority::Maintenance < BundlePriority::ScienceBulk);
    }

    #[test]
    fn test_max_delay_consistency() {
        // Les priorités plus hautes doivent avoir des délais plus courts
        assert!(
            BundlePriority::SafetyCritical.max_delay_seconds()
                < BundlePriority::Emergency.max_delay_seconds()
        );
        assert!(
            BundlePriority::Emergency.max_delay_seconds()
                < BundlePriority::Maintenance.max_delay_seconds()
        );
    }

    #[test]
    fn test_bundle_creation_and_expiry() {
        let bundle = PriorityBundle::new(
            crate::BundleId("test-bundle".into()),
            BundlePriority::ScienceNormal,
            "rover-01".into(),
            BundleDestination::EarthStation("dsn".into()),
            vec![0u8; 1024],
            3600,
        );

        assert!(!bundle.is_expired());
        assert!(bundle.remaining_ttl_seconds() > 3500);
        assert!((bundle.estimated_transmission_time_seconds(1.0) - 8.192).abs() < 0.01);
    }

    #[test]
    fn test_expired_bundle() {
        let mut bundle = PriorityBundle::new(
            crate::BundleId("expired".into()),
            BundlePriority::Maintenance,
            "rover-01".into(),
            BundleDestination::Relay("orbiter".into()),
            vec![],
            1,
        );

        bundle.expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(bundle.is_expired());
    }
}
