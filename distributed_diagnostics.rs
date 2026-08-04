//! # Diagnostics Distribués
//!
//! Système de diagnostic qui coordonne les vérifications de santé
//! entre tous les noeuds du réseau Mars. Permet la détection
//! d'anomalies distribuées (corrélation entre rovers, habitat, orbiteur).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{ComponentId, HealthMetric, HealthStatus, Anomaly, AnomalySeverity};

/// Résultat d'un diagnostic distribué
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedDiagnosis {
    pub diagnosis_id: String,
    pub timestamp: DateTime<Utc>,
    pub participating_nodes: Vec<String>,
    pub findings: Vec<DiagnosisFinding>,
    pub overall_health: f64, // 0.0-1.0
    pub recommendations: Vec<String>,
    pub correlated_anomalies: Vec<String>, // IDs d'anomalies corrélées
}

/// Résultat d'un diagnostic sur un noeud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisFinding {
    pub node_id: String,
    pub component: ComponentId,
    pub status: HealthStatus,
    pub confidence: f64, // 0.0-1.0
    pub detail: String,
}

/// Requête de diagnostic envoyée à un pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticQuery {
    pub query_id: String,
    pub from_node: String,
    pub target_components: Vec<ComponentId>,
    pub timeout_seconds: u32,
    pub priority: u8,
}

/// Réponse de diagnostic d'un pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResponse {
    pub query_id: String,
    pub from_node: String,
    pub timestamp: DateTime<Utc>,
    pub findings: Vec<DiagnosisFinding>,
    pub response_time_ms: u64,
}

/// Moteur de diagnostics distribués
pub struct DistributedDiagnostics {
    local_node_id: String,
    query_timeout_ms: u32,
    correlation_window_seconds: u64,
}

impl DistributedDiagnostics {
    /// Crée un nouveau moteur de diagnostics
    pub fn new(local_node_id: String) -> Self {
        Self {
            local_node_id,
            query_timeout_ms: 5000, // 5 secondes par défaut
            correlation_window_seconds: 300, // 5 minutes pour corrélation
        }
    }

    /// Lance un diagnostic distribué sur un ensemble de composants
    pub fn run_distributed_diagnosis(
        &self,
        local_findings: &[DiagnosisFinding],
        peer_responses: &[DiagnosticResponse],
    ) -> DistributedDiagnosis {
        let all_findings: Vec<DiagnosisFinding> = local_findings
            .iter()
            .cloned()
            .chain(
                peer_responses
                    .iter()
                    .flat_map(|r| r.findings.iter().cloned()),
            )
            .collect();

        // Calculer la santé globale
        let total = all_findings.len();
        let healthy = all_findings
            .iter()
            .filter(|f| f.status == HealthStatus::Nominal)
            .count();
        let overall_health = if total > 0 {
            (healthy as f64 / total as f64) * 100.0
        } else {
            100.0
        };

        // Générer des recommandations basées sur les findings
        let mut recommendations = Vec::new();
        let impaired = all_findings.iter().filter(|f| f.status != HealthStatus::Nominal).collect::<Vec<_>>();

        if impaired.len() > total / 4 {
            recommendations.push("HIGH: Consider entering safe mode - multiple components degraded".into());
        }

        for finding in &impaired {
            recommendations.push(format!(
                "Component {} on {} is {:?} - recommend maintenance",
                finding.component, finding.node_id, finding.status
            ));
        }

        if overall_health < 70.0 {
            recommendations.push("CRITICAL: System health below 70% - immediate action required".into());
        }

        // Collecter les participants
        let mut participants = vec![self.local_node_id.clone()];
        for resp in peer_responses {
            if !participants.contains(&resp.from_node) {
                participants.push(resp.from_node.clone());
            }
        }

        DistributedDiagnosis {
            diagnosis_id: format!(
                "diag-{}-{}",
                self.local_node_id,
                Utc::now().format("%Y%m%d%H%M%S")
            ),
            timestamp: Utc::now(),
            participating_nodes: participants,
            findings: all_findings,
            overall_health,
            recommendations,
            correlated_anomalies: Vec::new(),
        }
    }

    /// Corrèle les anomalies entre plusieurs noeuds
    pub fn correlate_anomalies(
        &self,
        anomalies_by_node: &HashMap<String, Vec<Anomaly>>,
    ) -> Vec<CorrelatedAnomaly> {
        let now = Utc::now();
        let window = chrono::Duration::seconds(self.correlation_window_seconds as i64);

        let mut correlated = Vec::new();

        // Regrouper par type de composant
        let mut by_component_type: HashMap<String, Vec<(&str, &Anomaly)>> = HashMap::new();
        for (node_id, anomalies) in anomalies_by_node {
            for anomaly in anomalies {
                let key = format!("{}", anomaly.component);
                by_component_type
                    .entry(key)
                    .or_default()
                    .push((node_id.as_str(), anomaly));
            }
        }

        // Trouver les corrélations (même composant, même fenêtre temporelle)
        for (component_type, entries) in &by_component_type {
            if entries.len() >= 2 {
                // Vérifier si les timestamps sont dans la fenêtre
                let times: Vec<DateTime<Utc>> = entries.iter().map(|(_, a)| a.detected_at).collect();
                let min_time = times.iter().min().unwrap();
                let max_time = times.iter().max().unwrap();

                if max_time.signed_duration_since(*min_time) <= window {
                    correlated.push(CorrelatedAnomaly {
                        component_type: component_type.clone(),
                        affected_nodes: entries.iter().map(|(n, _)| n.to_string()).collect(),
                        severity: entries
                            .iter()
                            .map(|(_, a)| &a.severity)
                            .max()
                            .unwrap()
                            .clone(),
                        time_window_secs: max_time.signed_duration_since(*min_time).num_seconds() as u32,
                        conclusion: format!(
                            "Correlated {} anomalies across {} nodes within {}s window",
                            component_type,
                            entries.len(),
                            max_time.signed_duration_since(*min_time).num_seconds()
                        ),
                    });
                }
            }
        }

        correlated
    }

    /// Lance un diagnostic de heartbeat sur tous les pairs connus
    pub fn heartbeat_diagnosis(&self, peer_count: usize, timeout_count: usize) -> DiagnosisFinding {
        let status = if timeout_count == 0 {
            HealthStatus::Nominal
        } else if timeout_count <= peer_count / 4 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Impaired
        };

        DiagnosisFinding {
            node_id: self.local_node_id.clone(),
            component: ComponentId("network_connectivity".into()),
            status,
            confidence: 1.0 - (timeout_count as f64 / peer_count.max(1) as f64),
            detail: format!(
                "Heartbeat: {}/{} peers responded ({} timeouts)",
                peer_count - timeout_count,
                peer_count,
                timeout_count
            ),
        }
    }
}

/// Anomalie corrélée entre plusieurs noeuds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedAnomaly {
    pub component_type: String,
    pub affected_nodes: Vec<String>,
    pub severity: AnomalySeverity,
    pub time_window_secs: u32,
    pub conclusion: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_diagnosis() {
        let engine = DistributedDiagnostics::new("rover-01".into());

        let local = vec![
            DiagnosisFinding {
                node_id: "rover-01".into(),
                component: ComponentId("processor".into()),
                status: HealthStatus::Nominal,
                confidence: 0.95,
                detail: "CPU temp normal".into(),
            },
        ];

        let peers = vec![
            DiagnosticResponse {
                query_id: "q1".into(),
                from_node: "habitat-01".into(),
                timestamp: Utc::now(),
                findings: vec![DiagnosisFinding {
                    node_id: "habitat-01".into(),
                    component: ComponentId("power".into()),
                    status: HealthStatus::Degraded,
                    confidence: 0.8,
                    detail: "Battery at 30%".into(),
                }],
                response_time_ms: 1200,
            },
        ];

        let diagnosis = engine.run_distributed_diagnosis(&local, &peers);
        assert_eq!(diagnosis.participating_nodes.len(), 2);
        assert_eq!(diagnosis.findings.len(), 2);
        assert!(diagnosis.overall_health < 100.0); // Habitat dégradé
    }

    #[test]
    fn test_correlate_anomalies() {
        let engine = DistributedDiagnostics::new("coordinator".into());

        let mut by_node = HashMap::new();
        by_node.insert(
            "rover-01".into(),
            vec![Anomaly {
                id: "a1".into(),
                component: ComponentId("power".into()),
                severity: AnomalySeverity::Warning,
                description: "Voltage drop".into(),
                detected_at: Utc::now(),
                related_metrics: vec![],
                recommended_action: "Check".into(),
            }],
        );
        by_node.insert(
            "habitat-01".into(),
            vec![Anomaly {
                id: "a2".into(),
                component: ComponentId("power".into()),
                severity: AnomalySeverity::Warning,
                description: "Voltage drop".into(),
                detected_at: Utc::now(),
                related_metrics: vec![],
                recommended_action: "Check".into(),
            }],
        );

        let correlated = engine.correlate_anomalies(&by_node);
        assert_eq!(correlated.len(), 1);
        assert_eq!(correlated[0].affected_nodes.len(), 2);
    }
}
