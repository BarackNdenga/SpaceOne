//! # Planificateur de Contingence
//!
//! Génère des plans alternatifs en cas d'échec de tâches, de perte
//! de ressources ou de situations d'urgence. Le planificateur évalue
//! les alternatives disponibles et propose la meilleure option.

use crate::{MarsTask, TaskId, TaskCategory, TaskStatus, ResourceRequirement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Niveau de sévérité de la situation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeverityLevel {
    Low,       // Impact mineur, pas d'urgence
    Moderate,  // Impact significatif, replanification nécessaire
    High,      // Impact critique, action immédiate requise
    Critical,  // Danger pour la mission ou l'équipage
}

/// Type d'anomalie déclenchant la contingence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    TaskFailure { task_id: TaskId, error_code: String },
    ResourceLoss { resource: String, lost_amount: f64 },
    CommunicationBlackout { expected_duration_secs: u32 },
    HardwareDegradation { component: String, remaining_life_pct: f64 },
    EnvironmentalHazard { hazard_type: String, duration_secs: u32 },
    CrewEmergency,
}

/// Un plan de contingence proposé
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContingencyPlan {
    pub plan_id: String,
    pub triggered_by: AnomalyType,
    pub severity: SeverityLevel,
    pub affected_tasks: Vec<TaskId>,
    pub actions: Vec<ContingencyAction>,
    pub estimated_impact: ImpactAssessment,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Action de contingence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContingencyAction {
    /// Replanifier une tâche avec un créneau différent
    Reschedule {
        task_id: TaskId,
        reason: String,
        delay_seconds: u32,
        new_priority: Option<u16>,
    },
    /// Remplacer une tâche par une alternative
    Substitute {
        original_task: TaskId,
        substitute_task: TaskId,
        substitution_reason: String,
    },
    /// Entrer en mode safe
    EnterSafeMode {
        reason: String,
        affected_systems: Vec<String>,
    },
    /// Réallouer des ressources
    ReallocateResources {
        from_task: TaskId,
        to_task: TaskId,
        resources: Vec<String>,
    },
    /// Annuler des tâches non-critiques
    CancelNonCritical {
        categories: Vec<TaskCategory>,
        reason: String,
    },
    /// Activer des redondances
    ActivateRedundancy {
        system: String,
        redundant_path: String,
    },
    /// Notifier le mission control
    NotifyMissionControl {
        severity: SeverityLevel,
        summary: String,
        data_bundle: Vec<u8>,
    },
}

/// Évaluation d'impact d'un plan de contingence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub scientific_value_loss: f64,      // 0.0-1.0
    pub energy_impact_wh: f64,           // Wh supplémentaires consommés
    pub time_impact_minutes: f64,         // Retard total
    pub crew_risk_level: f64,            // 0.0-1.0
    pub mission_continuity_pct: f64,     // % de mission maintenu
}

/// Le planificateur de contingence
pub struct ContingencyPlanner {
    max_plans_per_event: usize,
}

impl ContingencyPlanner {
    /// Crée un nouveau planificateur
    pub fn new() -> Self {
        Self {
            max_plans_per_event: 3,
        }
    }

    /// Génère un plan de contingence pour une tâche défaillante
    pub fn plan(&self, failed_task: &MarsTask, all_tasks: &HashMap<TaskId, MarsTask>) -> Option<ContingencyPlan> {
        let anomaly = AnomalyType::TaskFailure {
            task_id: failed_task.id.clone(),
            error_code: "TASK_FAILURE".into(),
        };

        let severity = self.assess_severity(failed_task);

        let mut actions = Vec::new();

        // Action 1: Tenter de replanifier la tâche
        actions.push(ContingencyAction::Reschedule {
            task_id: failed_task.id.clone(),
            reason: format!("Retrying failed task: {}", failed_task.name),
            delay_seconds: 600, // 10 minutes
            new_priority: Some(failed_task.priority.saturating_add(10)),
        });

        // Action 2: Si la tâche est critique, activer les redondances
        if failed_task.category == TaskCategory::Emergency || failed_task.category == TaskCategory::LifeSupport {
            actions.push(ContingencyAction::ActivateRedundancy {
                system: failed_task.category_name(),
                redundant_path: format!("backup_{}", failed_task.category_name()),
            });
        }

        // Action 3: Évaluer les tâches dépendantes
        let dependent_tasks: Vec<&MarsTask> = all_tasks
            .values()
            .filter(|t| t.dependencies.contains(&failed_task.id))
            .collect();

        if !dependent_tasks.is_empty() {
            actions.push(ContingencyAction::Reschedule {
                task_id: dependent_tasks[0].id.clone(),
                reason: format!("Dependent task blocked by failure of {}", failed_task.id),
                delay_seconds: 1200,
                new_priority: None,
            });
        }

        // Action 4: Notifier le mission control si sévérité élevée
        if severity == SeverityLevel::High || severity == SeverityLevel::Critical {
            actions.push(ContingencyAction::NotifyMissionControl {
                severity: severity.clone(),
                summary: format!("Task {} failed with severity {:?}", failed_task.id, severity),
                data_bundle: vec![], // Serait sérialisé en production
            });
        }

        let plan_id = format!(
            "contingency-{}-{}",
            failed_task.id,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );

        Some(ContingencyPlan {
            plan_id,
            triggered_by: anomaly,
            severity,
            affected_tasks: vec![failed_task.id.clone()],
            actions,
            estimated_impact: ImpactAssessment {
                scientific_value_loss: if failed_task.category == TaskCategory::Science { 0.3 } else { 0.1 },
                energy_impact_wh: 5.0,
                time_impact_minutes: 10.0,
                crew_risk_level: match severity {
                    SeverityLevel::Critical => 0.9,
                    SeverityLevel::High => 0.6,
                    SeverityLevel::Moderate => 0.3,
                    SeverityLevel::Low => 0.0,
                },
                mission_continuity_pct: match severity {
                    SeverityLevel::Critical => 70.0,
                    SeverityLevel::High => 85.0,
                    SeverityLevel::Moderate => 95.0,
                    SeverityLevel::Low => 99.0,
                },
            },
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        })
    }

    /// Évalue la sévérité d'une tâche défaillante
    fn assess_severity(&self, task: &MarsTask) -> SeverityLevel {
        match task.category {
            TaskCategory::Emergency => SeverityLevel::Critical,
            TaskCategory::LifeSupport => SeverityLevel::Critical,
            TaskCategory::Communication => SeverityLevel::High,
            TaskCategory::Science if task.scientific_value > 0.8 => SeverityLevel::High,
            TaskCategory::Science => SeverityLevel::Moderate,
            TaskCategory::Maintenance => SeverityLevel::Low,
            TaskCategory::Navigation => SeverityLevel::Moderate,
        }
    }

    /// Génère un plan pour une perte de ressources
    pub fn plan_resource_loss(
        &self,
        resource: &str,
        lost_amount: f64,
        all_tasks: &HashMap<TaskId, MarsTask>,
    ) -> Option<ContingencyPlan> {
        let anomaly = AnomalyType::ResourceLoss {
            resource: resource.into(),
            lost_amount,
        };

        // Trouver les tâches affectées
        let affected: Vec<TaskId> = all_tasks
            .values()
            .filter(|t| {
                t.required_resources.iter().any(|r| {
                    r.resource_type == resource && r.critical
                })
            })
            .map(|t| t.id.clone())
            .collect();

        let severity = if affected.len() > 3 {
            SeverityLevel::High
        } else if affected.is_empty() {
            SeverityLevel::Low
        } else {
            SeverityLevel::Moderate
        };

        let mut actions = Vec::new();

        // Réallouer des ressources des tâches non-critiques
        let non_critical: Vec<&MarsTask> = all_tasks
            .values()
            .filter(|t| t.category == TaskCategory::Science || t.category == TaskCategory::Navigation)
            .collect();

        if !affected.is_empty() && !non_critical.is_empty() {
            actions.push(ContingencyAction::ReallocateResources {
                from_task: non_critical[0].id.clone(),
                to_task: affected[0].clone(),
                resources: vec![resource.into()],
            });
        }

        if severity >= SeverityLevel::High {
            actions.push(ContingencyAction::EnterSafeMode {
                reason: format!("Critical resource loss: {} ({} lost)", resource, lost_amount),
                affected_systems: affected.iter().map(|id| id.0.clone()).collect(),
            });
        }

        Some(ContingencyPlan {
            plan_id: format!("contingency-resource-{}", Utc::now().format("%Y%m%d%H%M%S")),
            triggered_by: anomaly,
            severity,
            affected_tasks: affected,
            actions,
            estimated_impact: ImpactAssessment {
                scientific_value_loss: 0.2,
                energy_impact_wh: 0.0,
                time_impact_minutes: 30.0,
                crew_risk_level: if severity == SeverityLevel::Critical { 0.8 } else { 0.1 },
                mission_continuity_pct: if severity == SeverityLevel::Critical { 60.0 } else { 90.0 },
            },
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(30),
        })
    }
}

/// Extension pour obtenir le nom d'une catégorie
impl TaskCategory {
    pub fn category_name(&self) -> String {
        match self {
            TaskCategory::Science => "science".into(),
            TaskCategory::Navigation => "navigation".into(),
            TaskCategory::Maintenance => "maintenance".into(),
            TaskCategory::Communication => "communication".into(),
            TaskCategory::LifeSupport => "life_support".into(),
            TaskCategory::Emergency => "emergency".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskCategory;

    fn make_task_map() -> HashMap<TaskId, MarsTask> {
        let mut tasks = HashMap::new();

        tasks.insert(TaskId("science-1".into()), MarsTask {
            id: TaskId("science-1".into()),
            name: "Soil Analysis".into(),
            category: TaskCategory::Science,
            priority: 70,
            deadline: None,
            estimated_duration_secs: 600,
            dependencies: vec![],
            required_resources: vec![
                ResourceRequirement { resource_type: "power".into(), amount: 30.0, unit: "W".into(), critical: true },
            ],
            executing_node: None,
            status: TaskStatus::Scheduled,
            created_at: Utc::now(),
            scientific_value: 0.9,
        });

        tasks.insert(TaskId("nav-1".into()), MarsTask {
            id: TaskId("nav-1".into()),
            name: "Navigate to crater".into(),
            category: TaskCategory::Navigation,
            priority: 50,
            deadline: None,
            estimated_duration_secs: 1800,
            dependencies: vec![TaskId("science-1".into())],
            required_resources: vec![],
            executing_node: None,
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            scientific_value: 0.6,
        });

        tasks
    }

    #[test]
    fn test_contingency_plan_for_failed_task() {
        let planner = ContingencyPlanner::new();
        let tasks = make_task_map();
        let failed = tasks.get(&TaskId("science-1".into())).unwrap();

        let plan = planner.plan(failed, &tasks).unwrap();
        assert!(plan.actions.len() >= 2); // Reschedule + notify
        assert_eq!(plan.severity, SeverityLevel::High); // High science value
        assert!(plan.plan_id.starts_with("contingency-"));
    }

    #[test]
    fn test_resource_loss_plan() {
        let planner = ContingencyPlanner::new();
        let tasks = make_task_map();

        let plan = planner.plan_resource_loss("power", 50.0, &tasks).unwrap();
        assert_eq!(plan.severity, SeverityLevel::Moderate);
        assert!(plan.actions.len() >= 1);
    }

    #[test]
    fn test_emergency_task_severity() {
        let planner = ContingencyPlanner::new();
        let task = MarsTask {
            id: TaskId("emergency-1".into()),
            name: "Emergency shutdown".into(),
            category: TaskCategory::Emergency,
            priority: 100,
            deadline: None,
            estimated_duration_secs: 30,
            dependencies: vec![],
            required_resources: vec![],
            executing_node: None,
            status: TaskStatus::Failed,
            created_at: Utc::now(),
            scientific_value: 0.0,
        };

        let plan = planner.plan(&task, &HashMap::new()).unwrap();
        assert_eq!(plan.severity, SeverityLevel::Critical);
    }
}
