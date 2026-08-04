//! # Résolveur de Conflits
//!
//! Détecte et résout les conflits entre tâches simultanées :
//! - Conflits de ressources (énergie, bande passante, mécanismes)
//! - Conflits temporels (chevauchement de fenêtres d'exécution)
//! - Conflits spatiaux (deux rovers sur le même chemin)

use crate::{MarsTask, TaskId, TaskCategory, TaskStatus, ResourceRequirement};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Type de conflit détecté
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    /// Deux tâches nécessitent la même ressource au même moment
    ResourceConflict {
        resource: String,
        tasks: Vec<TaskId>,
        demand: f64,
        supply: f64,
    },
    /// Chevauchement temporel entre tâches
    TemporalOverlap {
        tasks: Vec<TaskId>,
        overlap_duration_secs: u32,
    },
    /// Conflit spatial (même zone physique)
    SpatialConflict {
        tasks: Vec<TaskId>,
        location: String,
    },
    /// Dépendance circulaire
    CircularDependency {
        cycle: Vec<TaskId>,
    },
}

/// Résolution proposée pour un conflit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    /// Séquentialiser les tâches (l'une après l'autre)
    Sequentialize {
        keep_order: Vec<TaskId>,
        delay_task: TaskId,
        delay_by_secs: u32,
    },
    /// Réduire les ressources d'une tâche
    ReduceScope {
        task_id: TaskId,
        reduced_resources: Vec<String>,
        quality_impact: f64, // 0.0-1.0, combien de qualité est perdue
    },
    /// Reporter une tâche à un moment ultérieur
    Reschedule {
        task_id: TaskId,
        new_window_start: DateTime<Utc>,
        new_window_end: DateTime<Utc>,
    },
    /// Annuler la tâche de plus basse priorité
    Cancel {
        task_id: TaskId,
        reason: String,
    },
    /// Paralléliser différemment (réassigner des ressources)
    Repartition {
        tasks: Vec<TaskId>,
        new_allocation: Vec<(TaskId, Vec<ResourceRequirement>)>,
    },
}

/// Résultat de résolution d'un conflit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolution {
    pub conflict_type: ConflictType,
    pub strategy: ResolutionStrategy,
    pub affected_tasks: Vec<TaskId>,
    pub priority_loss: f64, // Perte de priorité totale
    pub justification: String,
}

/// Le résolveur de conflits
pub struct ConflictResolver {
    max_conflicts_per_cycle: usize,
}

impl ConflictResolver {
    /// Crée un nouveau résolveur
    pub fn new() -> Self {
        Self {
            max_conflicts_per_cycle: 10,
        }
    }

    /// Crée un résolveur avec limite personnalisée
    pub fn with_limit(max_conflicts: usize) -> Self {
        Self {
            max_conflicts_per_cycle: max_conflicts,
        }
    }

    /// Détecte et résout les conflits dans une liste de tâches actives
    pub fn resolve(&self, tasks: &[&MarsTask]) -> Vec<ConflictResolution> {
        let conflicts = self.detect_conflicts(tasks);
        let mut resolutions = Vec::new();

        for conflict in conflicts.iter().take(self.max_conflicts_per_cycle) {
            if let Some(resolution) = self.resolve_conflict(conflict, tasks) {
                resolutions.push(resolution);
            }
        }

        resolutions
    }

    /// Détecte les conflits entre tâches
    fn detect_conflicts<'a>(&self, tasks: &[&'a MarsTask]) -> Vec<DetectedConflict<'a>> {
        let mut conflicts = Vec::new();

        // Détection des conflits de ressources
        for i in 0..tasks.len() {
            for j in (i + 1)..tasks.len() {
                let shared_resources = self.find_shared_resources(tasks[i], tasks[j]);
                if !shared_resources.is_empty() {
                    conflicts.push(DetectedConflict {
                        conflict_type: ConflictType::ResourceConflict {
                            resource: shared_resources[0].clone(),
                            tasks: vec![tasks[i].id.clone(), tasks[j].id.clone()],
                            demand: shared_resources.len() as f64,
                            supply: 1.0, // Assumons capacité unitaire
                        },
                        tasks_a: tasks[i],
                        tasks_b: tasks[j],
                    });
                }
            }
        }

        // Détection des conflits temporels (tâches sans deadline mais durées longues)
        for i in 0..tasks.len() {
            for j in (i + 1)..tasks.len() {
                if tasks[i].estimated_duration_secs + tasks[j].estimated_duration_secs > 3600
                    && tasks[i].status == TaskStatus::InProgress
                    && tasks[j].status == TaskStatus::InProgress
                {
                    conflicts.push(DetectedConflict {
                        conflict_type: ConflictType::TemporalOverlap {
                            tasks: vec![tasks[i].id.clone(), tasks[j].id.clone()],
                            overlap_duration_secs: tasks[j].estimated_duration_secs,
                        },
                        tasks_a: tasks[i],
                        tasks_b: tasks[j],
                    });
                }
            }
        }

        conflicts
    }

    /// Trouve les ressources partagées entre deux tâches
    fn find_shared_resources<'a>(&self, a: &'a MarsTask, b: &'a MarsTask) -> Vec<String> {
        let a_resources: Vec<&str> = a.required_resources.iter().map(|r| r.resource_type.as_str()).collect();
        let b_resources: Vec<&str> = b.required_resources.iter().map(|r| r.resource_type.as_str()).collect();

        a_resources
            .iter()
            .filter(|r| b_resources.contains(r))
            .map(|r| r.to_string())
            .collect()
    }

    /// Résout un conflit détecté
    fn resolve_conflict(&self, conflict: &DetectedConflict, tasks: &[&MarsTask]) -> Option<ConflictResolution> {
        let (a, b) = (conflict.tasks_a, conflict.tasks_b);

        // Stratégie : séquentialiser, la tâche de plus basse priorité est retardée
        if a.priority > b.priority {
            Some(ConflictResolution {
                conflict_type: conflict.conflict_type.clone(),
                strategy: ResolutionStrategy::Sequentialize {
                    keep_order: vec![a.id.clone(), b.id.clone()],
                    delay_task: b.id.clone(),
                    delay_by_secs: a.estimated_duration_secs,
                },
                affected_tasks: vec![a.id.clone(), b.id.clone()],
                priority_loss: 0.0,
                justification: format!(
                    "Task {} (priority {}) takes precedence over {} (priority {})",
                    a.id, a.priority, b.id, b.priority
                ),
            })
        } else if b.priority > a.priority {
            Some(ConflictResolution {
                conflict_type: conflict.conflict_type.clone(),
                strategy: ResolutionStrategy::Sequentialize {
                    keep_order: vec![b.id.clone(), a.id.clone()],
                    delay_task: a.id.clone(),
                    delay_by_secs: b.estimated_duration_secs,
                },
                affected_tasks: vec![a.id.clone(), b.id.clone()],
                priority_loss: 0.0,
                justification: format!(
                    "Task {} (priority {}) takes precedence over {} (priority {})",
                    b.id, b.priority, a.id, a.priority
                ),
            })
        } else {
            // Priorités égales : réduire le scope de la plus récente
            let newer = if a.created_at > b.created_at { a } else { b };
            Some(ConflictResolution {
                conflict_type: conflict.conflict_type.clone(),
                strategy: ResolutionStrategy::ReduceScope {
                    task_id: newer.id.clone(),
                    reduced_resources: newer.required_resources.iter().map(|r| r.resource_type.clone()).collect(),
                    quality_impact: 0.3,
                },
                affected_tasks: vec![a.id.clone(), b.id.clone()],
                priority_loss: 0.15,
                justification: format!("Equal priority, reducing scope of newer task {}", newer.id),
            })
        }
    }
}

/// Conflit détecté (interne)
struct DetectedConflict<'a> {
    conflict_type: ConflictType,
    tasks_a: &'a MarsTask,
    tasks_b: &'a MarsTask,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_task(id: &str, priority: u16, resources: Vec<ResourceRequirement>, duration: u32) -> MarsTask {
        MarsTask {
            id: TaskId(id.into()),
            name: id.into(),
            category: TaskCategory::Science,
            priority,
            deadline: None,
            estimated_duration_secs: duration,
            dependencies: vec![],
            required_resources: resources,
            executing_node: None,
            status: TaskStatus::InProgress,
            created_at: Utc::now(),
            scientific_value: 0.7,
        }
    }

    #[test]
    fn test_detect_resource_conflict() {
        let resolver = ConflictResolver::new();

        let t1 = make_task("task-a", 80, vec![
            ResourceRequirement { resource_type: "power".into(), amount: 50.0, unit: "W".into(), critical: true },
        ], 300);

        let t2 = make_task("task-b", 60, vec![
            ResourceRequirement { resource_type: "power".into(), amount: 40.0, unit: "W".into(), critical: true },
        ], 300);

        let tasks = vec![&t1, &t2];
        let resolutions = resolver.resolve(&tasks);

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(resolutions[0].strategy, ResolutionStrategy::Sequentialize { .. }));
    }

    #[test]
    fn test_no_conflict_different_resources() {
        let resolver = ConflictResolver::new();

        let t1 = make_task("task-a", 80, vec![
            ResourceRequirement { resource_type: "power".into(), amount: 50.0, unit: "W".into(), critical: true },
        ], 300);

        let t2 = make_task("task-b", 60, vec![
            ResourceRequirement { resource_type: "comms".into(), amount: 10.0, unit: "kbps".into(), critical: false },
        ], 300);

        let tasks = vec![&t1, &t2];
        let resolutions = resolver.resolve(&tasks);

        assert!(resolutions.is_empty());
    }

    #[test]
    fn test_higher_priority_precedence() {
        let resolver = ConflictResolver::new();

        let high = make_task("high-priority", 95, vec![
            ResourceRequirement { resource_type: "arm".into(), amount: 1.0, unit: "unit".into(), critical: true },
        ], 300);

        let low = make_task("low-priority", 30, vec![
            ResourceRequirement { resource_type: "arm".into(), amount: 1.0, unit: "unit".into(), critical: true },
        ], 300);

        let tasks = vec![&high, &low];
        let resolutions = resolver.resolve(&tasks);

        assert_eq!(resolutions.len(), 1);
        if let ResolutionStrategy::Sequentialize { delay_task, .. } = &resolutions[0].strategy {
            assert_eq!(delay_task.0, "low-priority");
        } else {
            panic!("Expected Sequentialize strategy");
        }
    }
}
