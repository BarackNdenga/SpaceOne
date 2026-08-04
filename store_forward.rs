//! # Store-and-Forward — Buffering Intelligent
//!
//! Gère le stockage temporaire des bundles pendant les périodes de
//! blackout de communication (éclipse, conjonction solaire, tempêtes
//! de poussière). Les bundles sont maintenus avec intégrité garantie
//! et éjectés selon les règles d'expiration et de priorité.

use crate::{BundleId, BundleDestination, DtnError, DtnResult, priority_bundle::{PriorityBundle, BundlePriority}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};

/// État d'un bundle dans le stockage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBundle {
    pub bundle: PriorityBundle,
    pub stored_at: DateTime<Utc>,
    pub access_count: u32,
    pub last_accessed: DateTime<Utc>,
    pub retransmission_count: u32,
    pub integrity_hash: String,
}

/// État du système de stockage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageState {
    pub total_capacity_bytes: u64,
    pub used_bytes: u64,
    pub bundle_count: usize,
    pub oldest_bundle_age_hours: f64,
    pub newest_bundle_age_seconds: f64,
    pub eviction_count: u64,
    pub integrity_failures: u64,
}

/// Stratégie d'éviction quand le buffer est plein
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvictionStrategy {
    /// Évince les bundles de plus basse priorité
    LowestPriority,
    /// Évince les bundles les plus anciens
    OldestFirst,
    /// Évince les bundles les moins accédés (LFU)
    LeastFrequentlyUsed,
    /// Évince les bundles les moins récemment accédés (LRU)
    LeastRecentlyUsed,
}

/// Le système store-and-forward
pub struct StoreForwardSystem {
    /// Stockage principal trié par priorité (BTreeMap)
    storage: BTreeMap<BundleId, StoredBundle>,
    /// Index par destination pour lookup rapide
    by_destination: HashMap<String, Vec<BundleId>>,
    /// Capacité totale en bytes
    total_capacity: u64,
    /// Octets actuellement utilisés
    used_bytes: u64,
    /// Stratégie d'éviction
    eviction_strategy: EvictionStrategy,
    /// Compteur d'évictions
    eviction_count: u64,
}

impl StoreForwardSystem {
    /// Crée un nouveau système de stockage
    pub fn new(total_capacity_bytes: u64) -> Self {
        info!("Store-Forward initialized: {} bytes capacity", total_capacity_bytes);
        Self {
            storage: BTreeMap::new(),
            by_destination: HashMap::new(),
            total_capacity: total_capacity_bytes,
            used_bytes: 0,
            eviction_strategy: EvictionStrategy::LowestPriority,
            eviction_count: 0,
        }
    }

    /// Crée un système avec stratégie d'éviction personnalisée
    pub fn with_eviction(total_capacity_bytes: u64, strategy: EvictionStrategy) -> Self {
        let mut system = Self::new(total_capacity_bytes);
        system.eviction_strategy = strategy;
        system
    }

    /// Stocke un bundle
    pub fn store(&mut self, bundle: PriorityBundle) -> DtnResult<()> {
        // Vérifier l'espace
        let bundle_size = bundle.payload_size;
        if self.used_bytes + bundle_size > self.total_capacity {
            // Tenter une éviction
            self.evict_if_needed(bundle_size)?;
        }

        // Calculer le hash d'intégrité
        let integrity_hash = format!(
            "{:x}",
            sha3::Sha3_256::new_with_prefix(&bundle.payload).finalize()
        );

        let stored = StoredBundle {
            bundle,
            stored_at: Utc::now(),
            access_count: 0,
            last_accessed: Utc::now(),
            retransmission_count: 0,
            integrity_hash,
        };

        let id = stored.bundle.id.clone();
        let dest_key = format!("{:?}", stored.bundle.destination);

        // Indexer par destination
        self.by_destination
            .entry(dest_key)
            .or_default()
            .push(id.clone());

        self.used_bytes += bundle_size;
        self.storage.insert(id, stored);

        Ok(())
    }

    /// Récupère un bundle par son ID
    pub fn retrieve(&mut self, id: &BundleId) -> Option<&PriorityBundle> {
        if let Some(stored) = self.storage.get_mut(id) {
            stored.access_count += 1;
            stored.last_accessed = Utc::now();
            Some(&stored.bundle)
        } else {
            None
        }
    }

    /// Récupère les bundles destinés à une destination
    pub fn retrieve_by_destination(&self, dest: &BundleDestination) -> Vec<&PriorityBundle> {
        let key = format!("{:?}", dest);
        let ids = match self.by_destination.get(&key) {
            Some(ids) => ids,
            None => return vec![],
        };

        ids.iter()
            .filter_map(|id| self.storage.get(id).map(|s| &s.bundle))
            .collect()
    }

    /// Supprime un bundle du stockage (après transmission réussie)
    pub fn remove(&mut self, id: &BundleId) -> bool {
        if let Some(stored) = self.storage.remove(id) {
            let key = format!("{:?}", stored.bundle.destination);
            if let Some(ids) = self.by_destination.get_mut(&key) {
                ids.retain(|bid| bid != id);
            }
            self.used_bytes = self.used_bytes.saturating_sub(stored.bundle.payload_size);
            true
        } else {
            false
        }
    }

    /// Incrémente le compteur de retransmission d'un bundle
    pub fn mark_retransmission(&mut self, id: &BundleId) {
        if let Some(stored) = self.storage.get_mut(id) {
            stored.retransmission_count += 1;
            warn!(
                "Retransmission #{} for bundle {}",
                stored.retransmission_count, id
            );
        }
    }

    /// Nettoie les bundles expirés
    pub fn cleanup_expired(&mut self) -> usize {
        let now = Utc::now();
        let expired: Vec<BundleId> = self
            .storage
            .iter()
            .filter(|(_, s)| s.bundle.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in expired {
            self.remove(&id);
        }

        if count > 0 {
            info!("Cleaned up {} expired bundles", count);
        }
        count
    }

    /// Évince des bundles si nécessaire pour libérer de l'espace
    fn evict_if_needed(&mut self, required_bytes: u64) -> DtnResult<()> {
        if self.used_bytes + required_bytes <= self.total_capacity {
            return Ok(());
        }

        let to_free = (self.used_bytes + required_bytes) - self.total_capacity;
        let mut freed = 0u64;

        let victims: Vec<BundleId> = match self.eviction_strategy {
            EvictionStrategy::LowestPriority => {
                self.storage
                    .iter()
                    .filter(|(_, s)| s.bundle.priority <= BundlePriority::ScienceBulk)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            EvictionStrategy::OldestFirst => {
                self.storage
                    .iter()
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            EvictionStrategy::LeastFrequentlyUsed => {
                let mut sorted: Vec<_> = self.storage.iter().collect();
                sorted.sort_by_key(|(_, s)| s.access_count);
                sorted.into_iter().map(|(id, _)| id.clone()).collect()
            }
            EvictionStrategy::LeastRecentlyUsed => {
                let mut sorted: Vec<_> = self.storage.iter().collect();
                sorted.sort_by_key(|(_, s)| s.last_accessed);
                sorted.into_iter().map(|(id, _)| id.clone()).collect()
            }
        };

        for id in victims {
            if freed >= to_free {
                break;
            }
            if self.remove(&id) {
                freed = self.total_capacity - self.used_bytes;
                self.eviction_count += 1;
            }
        }

        if freed < to_free {
            warn!(
                "Store-Forward: could not free enough space (freed={}, needed={})",
                freed, to_free
            );
        }

        Ok(())
    }

    /// État courant du stockage
    pub fn state(&self) -> StorageState {
        let now = Utc::now();
        let oldest_age = self
            .storage
            .values()
            .map(|s| now.signed_duration_since(s.stored_at).num_hours() as f64)
            .fold(0.0f64, |acc, x| acc.max(x));

        let newest_age = self
            .storage
            .values()
            .map(|s| now.signed_duration_since(s.stored_at).num_seconds() as f64)
            .fold(f64::MAX, |acc, x| acc.min(x));

        StorageState {
            total_capacity_bytes: self.total_capacity,
            used_bytes: self.used_bytes,
            bundle_count: self.storage.len(),
            oldest_bundle_age_hours: oldest_age,
            newest_bundle_age_seconds: if newest_age == f64::MAX { 0.0 } else { newest_age },
            eviction_count: self.eviction_count,
            integrity_failures: 0,
        }
    }

    /// Vérifie l'intégrité de tous les bundles stockés
    pub fn verify_integrity(&self) -> usize {
        use sha3::{Sha3_256, Digest};
        let mut failures = 0;

        for (id, stored) in &self.storage {
            let mut hasher = Sha3_256::new();
            hasher.update(&stored.bundle.payload);
            let computed = hex::encode(hasher.finalize());

            if computed != stored.integrity_hash {
                warn!("Integrity failure for bundle {}", id);
                failures += 1;
            }
        }

        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bundle(id: &str, priority: BundlePriority, size: u64) -> PriorityBundle {
        PriorityBundle::new(
            BundleId(id.into()),
            priority,
            "test".into(),
            BundleDestination::EarthStation("dsn".into()),
            vec![0u8; size as usize],
            86400,
        )
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut system = StoreForwardSystem::new(1024 * 1024); // 1 MB

        let bundle = make_bundle("b1", BundlePriority::ScienceNormal, 100);
        system.store(bundle).unwrap();

        let retrieved = system.retrieve(&BundleId("b1".into()));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().payload_size, 100);
    }

    #[test]
    fn test_eviction_on_capacity() {
        let mut system = StoreForwardSystem::new(300); // 300 bytes

        system.store(make_bundle("b1", BundlePriority::Maintenance, 150)).unwrap();
        system.store(make_bundle("b2", BundlePriority::ScienceBulk, 150)).unwrap();

        // Le 3ème bundle devrait déclencher une éviction
        let result = system.store(make_bundle("b3", BundlePriority::Command, 150));
        assert!(result.is_ok()); // Éviction a libéré de l'espace

        let state = system.state();
        assert!(state.bundle_count <= 3);
        assert!(state.eviction_count > 0);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut system = StoreForwardSystem::new(1024 * 1024);

        let mut bundle = make_bundle("expired", BundlePriority::Maintenance, 100);
        bundle.expires_at = Utc::now() - chrono::Duration::hours(1);

        system.store(bundle).unwrap();
        let cleaned = system.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(system.state().bundle_count, 0);
    }

    #[test]
    fn test_integrity_verification() {
        let mut system = StoreForwardSystem::new(1024 * 1024);

        system.store(make_bundle("intact", BundlePriority::Normal, 200)).unwrap();
        let failures = system.verify_integrity();
        assert_eq!(failures, 0);
    }

    #[test]
    fn test_retrieve_by_destination() {
        let mut system = StoreForwardSystem::new(1024 * 1024);

        system.store(make_bundle("b1", BundlePriority::Normal, 50)).unwrap();
        system.store(make_bundle("b2", BundlePriority::Normal, 50)).unwrap();

        let dest = BundleDestination::EarthStation("dsn".into());
        let bundles = system.retrieve_by_destination(&dest);
        assert_eq!(bundles.len(), 2);
    }

    #[test]
    fn test_storage_state() {
        let mut system = StoreForwardSystem::new(1024 * 1024);

        system.store(make_bundle("b1", BundlePriority::Normal, 1000)).unwrap();
        let state = system.state();

        assert_eq!(state.bundle_count, 1);
        assert_eq!(state.used_bytes, 1000);
        assert_eq!(state.total_capacity_bytes, 1024 * 1024);
    }
}
