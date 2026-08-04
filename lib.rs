//! # Health Management — Gestion de Santé Distribuée
//!
//! Système de monitoring, diagnostics, auto-recovery et safe mode
//! pour les plateformes martiennes. Détecte les anomalies, déclenche
//! les récupérations automatiques et supporte les décisions équipage.
//!
//! ## Architecture
//!
//! ```text
//! Sensors → Diagnostic Engine → Anomaly Detector → Recovery Actions
//!                │                      │                │
//!                └──── Heartbeat ───────┘                │
//!                         │                              │
//!                    Safe Mode Controller ←──────────────┘
//!                         │
//!                    Crew Decision Support
//! ```

pub mod distributed_diagnostics;
pub mod auto_recovery;
pub mod crew_decision_support;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use thiserror::Error;
use tracing::{info, warn, error, debug};

// ─── Types de Base ───

/// Identifiant d'un composant surveillé
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub String);

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type de composant martien
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType {
    // Système embarqué
    Processor,
    Memory,
    Power,
    Communication,
    Thermal,
    Navigation,

    // Payload scientifique
    Camera,
    Spectrometer,
    Drill,
    WeatherStation,

    // Support vie
    LifeSupport,
    AtmosphereControl,
    WaterRecycling,
    RadiationShield,

    // Propulsion (orbiteur)
    Thruster,
    FuelTank,
    ReactionWheel,
}

/// État de santé d'un composant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Fonctionnement nominal
    Nominal,
    /// Dégradation légère, surveillance renforcée
    Degraded,
    /// Fonctionnement impair, action requise
    Impaired,
    /// Composant défaillant, hors service
    Failed,
    /// Composant en recovery
    Recovering,
}

/// Métrique de santé
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetric {
    pub component_id: ComponentId,
    pub metric_name: String,
    pub value: f64,
    pub unit: String,
    pub threshold_warning: f64,
    pub threshold_critical: f64,
    pub timestamp: DateTime<Utc>,
}

impl HealthMetric {
    /// Évalue le statut basé sur les seuils
    pub fn evaluate(&self) -> HealthStatus {
        if self.value <= self.threshold_warning {
            HealthStatus::Nominal
        } else if self.value <= self.threshold_critical {
            HealthStatus::Degraded
        } else {
            HealthStatus::Impaired
        }
    }
}

/// Anomalie détectée
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub id: String,
    pub component: ComponentId,
    pub severity: AnomalySeverity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub related_metrics: Vec<HealthMetric>,
    pub recommended_action: String,
}

/// Sévérité d'une anomalie
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Mode de sécurité
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafeModeLevel {
    /// Mode nominal
    Nominal,
    /// Restrictions partielles
    Restricted,
    /// Mode safe minimum
    SafeMinimum,
    /// Arrêt d'urgence
    EmergencyShutdown,
}

// ─── Erreurs ───

#[derive(Error, Debug)]
pub enum HealthError {
    #[error("Composant non trouvé: {0}")]
    ComponentNotFound(ComponentId),

    #[error("Seuil critique dépassé: {0}")]
    CriticalThresholdExceeded(String),

    #[error("Recovery échouée: {0}")]
    RecoveryFailed(String),

    #[error("Safe mode activé: {0}")]
    SafeModeActivated(String),
}

pub type HealthResult<T> = Result<T, HealthError>;

// ─── Health Manager Principal ───

/// Gestionnaire de santé distribué
pub struct HealthManager {
    /// Composants surveillés
    components: Arc<RwLock<HashMap<ComponentId, ComponentHealth>>>,
    /// Historique des anomalies
    anomaly_log: Arc<RwLock<Vec<Anomaly>>>,
    /// Mode de sécurité courant
    safe_mode: Arc<RwLock<SafeModeLevel>>,
    /// Recovery en cours
    active_recoveries: Arc<RwLock<Vec<String>>>,
}

/// État de santé complet d'un composant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub id: ComponentId,
    pub component_type: ComponentType,
    pub status: HealthStatus,
    pub metrics: Vec<HealthMetric>,
    pub last_check: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub failure_count: u32,
    pub last_recovery: Option<DateTime<Utc>>,
}

impl HealthManager {
    /// Crée un nouveau gestionnaire de santé
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            anomaly_log: Arc::new(RwLock::new(Vec::new())),
            safe_mode: Arc::new(RwLock::new(SafeModeLevel::Nominal)),
            active_recoveries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Enregistre un composant à surveiller
    pub async fn register_component(&self, id: ComponentId, component_type: ComponentType) {
        let health = ComponentHealth {
            id: id.clone(),
            component_type,
            status: HealthStatus::Nominal,
            metrics: Vec::new(),
            last_check: Utc::now(),
            uptime_seconds: 0,
            failure_count: 0,
            last_recovery: None,
        };

        self.components.write().await.insert(id.clone(), health);
        info!("Registered health component: {}", id);
    }

    /// Met à jour une métrique de santé
    pub async fn update_metric(&self, metric: HealthMetric) -> HealthResult<()> {
        let mut components = self.components.write().await;
        let component = components
            .get_mut(&metric.component_id)
            .ok_or_else(|| HealthError::ComponentNotFound(metric.component_id.clone()))?;

        // Ajouter la métrique (garder les 100 dernières)
        component.metrics.push(metric.clone());
        if component.metrics.len() > 100 {
            component.metrics.remove(0);
        }

        // Évaluer le statut
        let new_status = metric.evaluate();
        let old_status = component.status.clone();
        component.status = new_status.clone();
        component.last_check = Utc::now();

        // Détecter les anomalies
        if new_status == HealthStatus::Impaired || new_status == HealthStatus::Failed {
            let anomaly = Anomaly {
                id: format!("anomaly-{}-{}", metric.component_id, Utc::now().format("%s")),
                component: metric.component_id.clone(),
                severity: if new_status == HealthStatus::Failed {
                    AnomalySeverity::Emergency
                } else {
                    AnomalySeverity::Critical
                },
                description: format!(
                    "Metric '{}' exceeded threshold: {:.2} > {:.2} (critical)",
                    metric.metric_name, metric.value, metric.threshold_critical
                ),
                detected_at: Utc::now(),
                related_metrics: vec![metric],
                recommended_action: format!("Initiate recovery for {}", metric.component_id),
            };

            warn!("Anomaly detected: {} ({:?})", anomaly.id, anomaly.severity);
            self.anomaly_log.write().await.push(anomaly);

            // Incrémenter le compteur d'échecs
            component.failure_count += 1;
        } else if new_status == HealthStatus::Degraded && old_status == HealthStatus::Nominal {
            let anomaly = Anomaly {
                id: format!("anomaly-degraded-{}-{}", metric.component_id, Utc::now().format("%s")),
                component: metric.component_id.clone(),
                severity: AnomalySeverity::Warning,
                description: format!(
                    "Metric '{}' warning threshold: {:.2} > {:.2}",
                    metric.metric_name, metric.value, metric.threshold_warning
                ),
                detected_at: Utc::now(),
                related_metrics: vec![metric],
                recommended_action: format!("Monitor {} closely", metric.component_id),
            };

            self.anomaly_log.write().await.push(anomaly);
        }

        Ok(())
    }

    /// Récupère l'état de santé d'un composant
    pub async fn get_health(&self, component_id: &ComponentId) -> HealthResult<ComponentHealth> {
        self.components
            .read()
            .await
            .get(component_id)
            .cloned()
            .ok_or_else(|| HealthError::ComponentNotFound(component_id.clone()))
    }

    /// État de santé global du système
    pub async fn system_health(&self) -> SystemHealthReport {
        let components = self.components.read().await;
        let anomalies = self.anomaly_log.read().await;
        let safe_mode = self.safe_mode.read().await;

        let status_counts: HashMap<HealthStatus, usize> = components
            .values()
            .fold(HashMap::new(), |mut acc, c| {
                *acc.entry(c.status.clone()).or_insert(0) += 1;
                acc
            });

        let total = components.len();
        let nominal = status_counts.get(&HealthStatus::Nominal).copied().unwrap_or(0);
        let degraded = status_counts.get(&HealthStatus::Degraded).copied().unwrap_or(0);
        let impaired = status_counts.get(&HealthStatus::Impaired).copied().unwrap_or(0);
        let failed = status_counts.get(&HealthStatus::Failed).copied().unwrap_or(0);

        SystemHealthReport {
            total_components: total,
            nominal_count: nominal,
            degraded_count: degraded,
            impaired_count: impaired,
            failed_count: failed,
            health_pct: if total > 0 {
                (nominal as f64 / total as f64) * 100.0
            } else {
                100.0
            },
            active_anomalies: anomalies.len(),
            safe_mode_level: safe_mode.clone(),
        }
    }

    /// Active le safe mode
    pub async fn activate_safe_mode(&self, level: SafeModeLevel, reason: &str) {
        let mut safe_mode = self.safe_mode.write().await;
        warn!("Activating safe mode {:?}: {}", level, reason);
        *safe_mode = level;

        // Désactiver les composants non-critiques
        let mut components = self.components.write().await;
        for (_, comp) in components.iter_mut() {
            if comp.status == HealthStatus::Nominal {
                // En safe mode, surveiller plus strictement
                if level == SafeModeLevel::SafeMinimum || level == SafeModeLevel::EmergencyShutdown {
                    comp.status = HealthStatus::Degraded; // Surveillance renforcée
                }
            }
        }
    }

    /// Liste des anomalies actives
    pub async fn active_anomalies(&self) -> Vec<Anomaly> {
        self.anomaly_log
            .read()
            .await
            .iter()
            .filter(|a| a.severity >= AnomalySeverity::Warning)
            .cloned()
            .collect()
    }

    /// Nettoie les anomalies anciennes (> 24h)
    pub async fn cleanup_old_anomalies(&self) -> usize {
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        let mut log = self.anomaly_log.write().await;
        let initial = log.len();
        log.retain(|a| a.detected_at > cutoff);
        initial - log.len()
    }
}

/// Rapport de santé global du système
#[derive(Debug, Clone, Serialize)]
pub struct SystemHealthReport {
    pub total_components: usize,
    pub nominal_count: usize,
    pub degraded_count: usize,
    pub impaired_count: usize,
    pub failed_count: usize,
    pub health_pct: f64,
    pub active_anomalies: usize,
    pub safe_mode_level: SafeModeLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_monitor() {
        let manager = HealthManager::new();

        manager
            .register_component(ComponentId("processor-01".into()), ComponentType::Processor)
            .await;

        let metric = HealthMetric {
            component_id: ComponentId("processor-01".into()),
            metric_name: "cpu_temperature".into(),
            value: 45.0,
            unit: "°C".into(),
            threshold_warning: 50.0,
            threshold_critical: 70.0,
            timestamp: Utc::now(),
        };

        manager.update_metric(metric).await.unwrap();

        let health = manager.get_health(&ComponentId("processor-01".into())).await.unwrap();
        assert_eq!(health.status, HealthStatus::Nominal);
    }

    #[tokio::test]
    async fn test_anomaly_detection() {
        let manager = HealthManager::new();

        manager
            .register_component(ComponentId("power-01".into()), ComponentType::Power)
            .await;

        // Métrique dépassant le seuil critique
        let metric = HealthMetric {
            component_id: ComponentId("power-01".into()),
            metric_name: "voltage_drop".into(),
            value: 85.0, // Très haut
            unit: "mV".into(),
            threshold_warning: 50.0,
            threshold_critical: 80.0,
            timestamp: Utc::now(),
        };

        manager.update_metric(metric).await.unwrap();

        let anomalies = manager.active_anomalies().await;
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].severity, AnomalySeverity::Critical);
    }

    #[tokio::test]
    async fn test_system_health_report() {
        let manager = HealthManager::new();

        manager
            .register_component(ComponentId("comp-1".into()), ComponentType::Processor)
            .await;
        manager
            .register_component(ComponentId("comp-2".into()), ComponentType::Memory)
            .await;

        let report = manager.system_health().await;
        assert_eq!(report.total_components, 2);
        assert_eq!(report.nominal_count, 2);
        assert_eq!(report.health_pct, 100.0);
    }

    #[tokio::test]
    async fn test_safe_mode_activation() {
        let manager = HealthManager::new();
        manager.activate_safe_mode(SafeModeLevel::SafeMinimum, "Test").await;

        let report = manager.system_health().await;
        assert_eq!(report.safe_mode_level, SafeModeLevel::SafeMinimum);
    }
}
