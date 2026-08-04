//! # Auto-Recovery — Récupération Automatique
//!
//! Système de récupération automatique qui tente de restaurer les
//! composants défaillants sans intervention humaine. Suit une stratégie
//! en escalier : restart soft → restart hard → failover → safe mode.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{ComponentId, ComponentType, HealthStatus};

/// Stratégie de recovery (escalier)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Redémarrage logiciel du composant
    SoftRestart,
    /// Redémarrage matériel (power cycle)
    HardRestart,
    /// Bascule vers le composant redondant
    Failover,
    /// Isoler le composant et réallouer
    Isolate,
    /// Entrer en safe mode
    EnterSafeMode,
}

/// Résultat d'une tentative de recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub component_id: ComponentId,
    pub strategy: RecoveryStrategy,
    pub attempt: u32,
    pub success: bool,
    pub duration_ms: u64,
    pub detail: String,
    pub new_status: HealthStatus,
}

/// Historique de recovery d'un composant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryHistory {
    pub component_id: ComponentId,
    pub total_attempts: u32,
    pub successful: u32,
    pub last_strategy: Option<RecoveryStrategy>,
    pub last_success: bool,
    pub last_recovery_at: Option<DateTime<Utc>>,
    pub average_recovery_time_ms: f64,
}

/// Moteur d'auto-recovery
pub struct AutoRecoveryEngine {
    /// Stratégie par type de composant
    strategy_map: HashMap<ComponentType, RecoveryStrategy>,
    /// Nombre max de tentatives avant escalade
    max_attempts_per_level: u32,
    /// Historique par composant
    history: HashMap<ComponentId, RecoveryHistory>,
}

impl AutoRecoveryEngine {
    /// Crée un moteur avec stratégies par défaut
    pub fn new() -> Self {
        let mut strategy_map = HashMap::new();
        strategy_map.insert(ComponentType::Processor, RecoveryStrategy::SoftRestart);
        strategy_map.insert(ComponentType::Memory, RecoveryStrategy::HardRestart);
        strategy_map.insert(ComponentType::Power, RecoveryStrategy::Failover);
        strategy_map.insert(ComponentType::Communication, RecoveryStrategy::SoftRestart);
        strategy_map.insert(ComponentType::LifeSupport, RecoveryStrategy::EnterSafeMode);
        strategy_map.insert(ComponentType::Thruster, RecoveryStrategy::Isolate);
        strategy_map.insert(ComponentType::Camera, RecoveryStrategy::SoftRestart);
        strategy_map.insert(ComponentType::Navigation, RecoveryStrategy::HardRestart);

        Self {
            strategy_map,
            max_attempts_per_level: 3,
            history: HashMap::new(),
        }
    }

    /// Exécute une tentative de recovery (simulation)
    pub fn attempt_recovery(
        &mut self,
        component_id: &ComponentId,
        component_type: &ComponentType,
    ) -> RecoveryResult {
        let strategy = self.strategy_map
            .get(component_type)
            .cloned()
            .unwrap_or(RecoveryStrategy::SoftRestart);

        let start = std::time::Instant::now();

        // Simuler le recovery
        let (success, detail, new_status) = match strategy {
            RecoveryStrategy::SoftRestart => (true, "Soft restart completed".into(), HealthStatus::Nominal),
            RecoveryStrategy::HardRestart => (true, "Hard restart (power cycle) completed".into(), HealthStatus::Nominal),
            RecoveryStrategy::Failover => (true, "Failed over to redundant component".into(), HealthStatus::Nominal),
            RecoveryStrategy::Isolate => (true, "Component isolated, resources reallocated".into(), HealthStatus::Impaired),
            RecoveryStrategy::EnterSafeMode => (true, "Safe mode activated".into(), HealthStatus::Degraded),
        };

        let duration = start.elapsed().as_millis() as u64;

        // Mettre à jour l'historique
        let hist = self.history.entry(component_id.clone()).or_insert_with(|| {
            RecoveryHistory {
                component_id: component_id.clone(),
                total_attempts: 0,
                successful: 0,
                last_strategy: None,
                last_success: false,
                last_recovery_at: None,
                average_recovery_time_ms: 0.0,
            }
        });

        hist.total_attempts += 1;
        if success {
            hist.successful += 1;
        }
        hist.last_strategy = Some(strategy.clone());
        hist.last_success = success;
        hist.last_recovery_at = Some(Utc::now());

        let result = RecoveryResult {
            component_id: component_id.clone(),
            strategy,
            attempt: hist.total_attempts,
            success,
            duration_ms: duration,
            detail,
            new_status,
        };

        result
    }

    /// Détermine la prochaine stratégie (escalade)
    pub fn escalate_strategy(
        &self,
        component_id: &ComponentId,
        current_strategy: &RecoveryStrategy,
    ) -> RecoveryStrategy {
        // Ordre d'escalade: SoftRestart → HardRestart → Failover → Isolate → SafeMode
        let escalation_order = [
            RecoveryStrategy::SoftRestart,
            RecoveryStrategy::HardRestart,
            RecoveryStrategy::Failover,
            RecoveryStrategy::Isolate,
            RecoveryStrategy::EnterSafeMode,
        ];

        let current_idx = escalation_order
            .iter()
            .position(|s| s == current_strategy)
            .unwrap_or(0);

        if current_idx + 1 < escalation_order.len() {
            escalation_order[current_idx + 1].clone()
        } else {
            RecoveryStrategy::EnterSafeMode // Dernier recours
        }
    }

    /// Historique d'un composant
    pub fn get_history(&self, component_id: &ComponentId) -> Option<&RecoveryHistory> {
        self.history.get(component_id)
    }

    /// Statistiques globales de recovery
    pub fn statistics(&self) -> RecoveryStats {
        let total: u32 = self.history.values().map(|h| h.total_attempts).sum();
        let successful: u32 = self.history.values().map(|h| h.successful).sum();

        RecoveryStats {
            total_components_recovered: self.history.len(),
            total_attempts: total,
            success_rate: if total > 0 { successful as f64 / total as f64 } else { 1.0 },
        }
    }
}

/// Statistiques de recovery
#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub total_components_recovered: usize,
    pub total_attempts: u32,
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_attempt() {
        let mut engine = AutoRecoveryEngine::new();
        let comp_id = ComponentId("processor-01".into());

        let result = engine.attempt_recovery(&comp_id, &ComponentType::Processor);
        assert!(result.success);
        assert_eq!(result.new_status, HealthStatus::Nominal);
        assert_eq!(result.attempt, 1);
    }

    #[test]
    fn test_escalation() {
        let engine = AutoRecoveryEngine::new();
        let comp_id = ComponentId("test".into());

        let next = engine.escalate_strategy(&comp_id, &RecoveryStrategy::SoftRestart);
        assert_eq!(next, RecoveryStrategy::HardRestart);

        let next = engine.escalate_strategy(&comp_id, &RecoveryStrategy::EnterSafeMode);
        assert_eq!(next, RecoveryStrategy::EnterSafeMode); // Déjà au max
    }

    #[test]
    fn test_recovery_history() {
        let mut engine = AutoRecoveryEngine::new();
        let comp_id = ComponentId("memory-01".into());

        for _ in 0..3 {
            engine.attempt_recovery(&comp_id, &ComponentType::Memory);
        }

        let hist = engine.get_history(&comp_id).unwrap();
        assert_eq!(hist.total_attempts, 3);
        assert_eq!(hist.successful, 3);
    }
}
