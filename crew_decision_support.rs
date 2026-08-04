//! # Crew Decision Support — Support Décisionnel Équipage
//!
//! Fournit à l'équipage des recommandations structurées et contextualisées
//! pour les décisions critiques. Présente les options, les conséquences
//! et les recommandations du système de santé.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{ComponentId, HealthStatus, Anomaly, AnomalySeverity, SafeModeLevel};

/// Option de décision proposée à l'équipage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    pub id: String,
    pub title: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub time_to_execute_secs: u32,
    pub reversibility: bool,
    pub expected_outcome: String,
    pub system_recommendation: bool, // Si le système recommande cette option
}

/// Niveau de risque
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Minimal,
    Low,
    Moderate,
    High,
    Critical,
}

/// Recommandation complète pour l'équipage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewRecommendation {
    pub id: String,
    pub title: String,
    pub severity: AnomalySeverity,
    pub situation_summary: String,
    pub affected_systems: Vec<ComponentId>,
    pub options: Vec<DecisionOption>,
    pub recommended_option: String,
    pub time_pressure: TimePressure,
    pub generated_at: DateTime<Utc>,
}

/// Pression temporelle pour la décision
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimePressure {
    /// Décision peut attendre (heures)
    Extended,
    /// Décision dans la fenêtre actuelle (minutes)
    Normal,
    /// Décision immédiate requise (secondes)
    Immediate,
}

/// Moteur de support décisionnel
pub struct CrewDecisionSupport {
    templates: HashMap<String, DecisionTemplate>,
}

/// Template de décision pour un type de situation
#[derive(Debug, Clone)]
struct DecisionTemplate {
    situation_type: String,
    default_options: Vec<DecisionOption>,
    escalation_threshold: AnomalySeverity,
}

impl CrewDecisionSupport {
    /// Crée un nouveau moteur de support décisionnel
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        // Template pour perte de puissance
        templates.insert(
            "power_loss".into(),
            DecisionTemplate {
                situation_type: "power_loss".into(),
                default_options: vec![
                    DecisionOption {
                        id: "power_reduce_load".into(),
                        title: "Réduire la charge électrique".into(),
                        description: "Désactiver les systèmes non-critiques (caméras, instruments non-essentiels)".into(),
                        risk_level: RiskLevel::Low,
                        time_to_execute_secs: 60,
                        reversibility: true,
                        expected_outcome: "Consommation réduite de 30%, systèmes critiques maintenus".into(),
                        system_recommendation: true,
                    },
                    DecisionOption {
                        id: "power_switch_source".into(),
                        title: "Basculer vers source secondaire".into(),
                        description: "Activer les batteries de secours ou le RTG secondaire".into(),
                        risk_level: RiskLevel::Moderate,
                        time_to_execute_secs: 120,
                        reversibility: true,
                        expected_outcome: "Alimentation restaurée via source redondante".into(),
                        system_recommendation: false,
                    },
                    DecisionOption {
                        id: "power_safe_mode".into(),
                        title: "Entrer en mode safe".into(),
                        description: "Arrêter tous les systèmes non-vitaux, contacter la Terre".into(),
                        risk_level: RiskLevel::High,
                        time_to_execute_secs: 30,
                        reversibility: true,
                        expected_outcome: "Survie garantie, mission scientifique suspendue".into(),
                        system_recommendation: false,
                    },
                ],
                escalation_threshold: AnomalySeverity::Critical,
            },
        );

        // Template pour anomalie thermique
        templates.insert(
            "thermal_anomaly".into(),
            DecisionTemplate {
                situation_type: "thermal_anomaly".into(),
                default_options: vec![
                    DecisionOption {
                        id: "thermal_adjust_setpoint".into(),
                        title: "Ajuster le point de consigne thermique".into(),
                        description: "Modifier la température cible du système de contrôle thermique".into(),
                        risk_level: RiskLevel::Low,
                        time_to_execute_secs: 300,
                        reversibility: true,
                        expected_outcome: "Stabilisation thermique progressive".into(),
                        system_recommendation: true,
                    },
                    DecisionOption {
                        id: "thermal_reduce_activity".into(),
                        title: "Réduire l'activité du système".into(),
                        description: "Limiter les opérations génératrices de chaleur".into(),
                        risk_level: RiskLevel::Moderate,
                        time_to_execute_secs: 600,
                        reversibility: true,
                        expected_outcome: "Réduction de la charge thermique".into(),
                        system_recommendation: false,
                    },
                ],
                escalation_threshold: AnomalySeverity::Warning,
            },
        );

        Self { templates }
    }

    /// Génère une recommandation pour l'équipage basée sur les anomalies actives
    pub fn generate_recommendation(
        &self,
        anomalies: &[Anomaly],
        safe_mode: &SafeModeLevel,
    ) -> Vec<CrewRecommendation> {
        let mut recommendations = Vec::new();

        // Grouper les anomalies par type de situation
        let mut grouped: HashMap<String, Vec<&Anomaly>> = HashMap::new();
        for anomaly in anomalies {
            let key = anomaly
                .description
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_lowercase();

            let template_key = match key.as_str() {
                "voltage" | "power" | "battery" => "power_loss".to_string(),
                "temperature" | "thermal" | "heat" => "thermal_anomaly".to_string(),
                _ => "general".to_string(),
            };

            grouped
                .entry(template_key)
                .or_default()
                .push(anomaly);
        }

        for (situation_type, group_anomalies) in &grouped {
            let template = self.templates.get(situation_type);

            let options = if let Some(t) = template {
                t.default_options.clone()
            } else {
                // Options génériques
                vec![
                    DecisionOption {
                        id: "general_monitor".into(),
                        title: "Continuer la surveillance".into(),
                        description: "Maintenir l'observation des paramètres affectés".into(),
                        risk_level: RiskLevel::Low,
                        time_to_execute_secs: 0,
                        reversibility: true,
                        expected_outcome: "Données supplémentaires pour décision éclairée".into(),
                        system_recommendation: true,
                    },
                    DecisionOption {
                        id: "general_safe_mode".into(),
                        title: "Entrer en safe mode".into(),
                        description: "Protéger les systèmes et contacter la Terre".into(),
                        risk_level: RiskLevel::High,
                        time_to_execute_secs: 30,
                        reversibility: true,
                        expected_outcome: "Sécurité maximale, mission suspendue".into(),
                        system_recommendation: false,
                    },
                ]
            };

            let max_severity = group_anomalies
                .iter()
                .map(|a| &a.severity)
                .max()
                .unwrap()
                .clone();

            let time_pressure = match max_severity {
                AnomalySeverity::Emergency => TimePressure::Immediate,
                AnomalySeverity::Critical => TimePressure::Normal,
                _ => TimePressure::Extended,
            };

            let recommended = options
                .iter()
                .find(|o| o.system_recommendation)
                .map(|o| o.id.clone())
                .unwrap_or_else(|| options[0].id.clone());

            let affected: Vec<ComponentId> = group_anomalies
                .iter()
                .map(|a| a.component.clone())
                .collect();

            recommendations.push(CrewRecommendation {
                id: format!("rec-{}-{}", situation_type, Utc::now().format("%s")),
                title: format!("Anomalie {}: {} système(s) affecté(s)", situation_type, group_anomalies.len()),
                severity: max_severity,
                situation_summary: group_anomalies
                    .iter()
                    .map(|a| a.description.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
                affected_systems: affected,
                options,
                recommended_option: recommended,
                time_pressure,
                generated_at: Utc::now(),
            });
        }

        recommendations
    }

    /// Évalue si une décision a été prise en temps
    pub fn evaluate_decision_timeliness(
        &self,
        decision_time: DateTime<Utc>,
        detection_time: DateTime<Utc>,
        severity: &AnomalySeverity,
    ) -> bool {
        let max_delay = match severity {
            AnomalySeverity::Emergency => chrono::Duration::seconds(30),
            AnomalySeverity::Critical => chrono::Duration::seconds(300),
            AnomalySeverity::Warning => chrono::Duration::minutes(15),
            AnomalySeverity::Info => chrono::Duration::hours(1),
        };

        decision_time <= detection_time + max_delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_recommendation() {
        let support = CrewDecisionSupport::new();

        let anomalies = vec![
            Anomaly {
                id: "a1".into(),
                component: ComponentId("power-01".into()),
                severity: AnomalySeverity::Critical,
                description: "Voltage drop detected on primary bus".into(),
                detected_at: Utc::now(),
                related_metrics: vec![],
                recommended_action: "Reduce load".into(),
            },
        ];

        let recs = support.generate_recommendation(&anomalies, &SafeModeLevel::Nominal);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].severity, AnomalySeverity::Critical);
        assert!(!recs[0].options.is_empty());
        assert!(recs[0].time_pressure == TimePressure::Normal);
    }

    #[test]
    fn test_decision_timeliness() {
        let support = CrewDecisionSupport::new();
        let detection = Utc::now();
        let decision_fast = detection + chrono::Duration::seconds(10);
        let decision_slow = detection + chrono::Duration::minutes(30);

        assert!(support.evaluate_decision_timeliness(
            decision_fast,
            detection,
            &AnomalySeverity::Emergency
        ));
        assert!(!support.evaluate_decision_timeliness(
            decision_slow,
            detection,
            &AnomalySeverity::Emergency
        ));
    }

    #[test]
    fn test_thermal_recommendation() {
        let support = CrewDecisionSupport::new();

        let anomalies = vec![Anomaly {
            id: "thermal-1".into(),
            component: ComponentId("thermal-01".into()),
            severity: AnomalySeverity::Warning,
            description: "Temperature anomaly on thermal control system".into(),
            detected_at: Utc::now(),
            related_metrics: vec![],
            recommended_action: "Adjust setpoint".into(),
        }];

        let recs = support.generate_recommendation(&anomalies, &SafeModeLevel::Nominal);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].options.len(), 2); // Template thermique
        assert_eq!(recs[0].time_pressure, TimePressure::Extended);
    }
}
