//! # Autonomous Scheduler
//!
//! Planificateur autonome embarqué pour la coordination des tâches martiennes.
//! Utilise un modèle ONNX (radiation-tolerant via TMR) pour la priorisation
//! et intègre un planificateur de contingence pour les situations d'urgence.
//!
//! ## Architecture
//!
//! Le scheduler fonctionne en 3 phases :
//! 1. **Priorisation** — Classe les tâches par urgence et impact scientifique
//! 2. **Résolution de conflits** — Détecte et résout les chevauchements
//! 3. **Plan de contingence** — Génère des plans alternatifs en cas d'anomalie

pub mod task_prioritizer;
pub mod conflict_resolver;
pub mod contingency_planner;

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use chrono::{DateTime, Utc};
use thiserror::Error;
use tracing::{info, warn, error};

// ─── Types de Tâches ───

/// Catégorie scientifique d'une tâche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskCategory {
    Science,        // Observation, échantillonnage, analyse
    Navigation,     // Déplacement, cartographie
    Maintenance,    // Calibration, nettoyage, diagnostic
    Communication,  // Transmission, relay, uplink
    LifeSupport,    // Énergie, air, eau, thermique
    Emergency,      // Safe mode, escape, shutdown
}

/// Statut d'une tâche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Scheduled,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    Contingency, // Répliquée par le plan de contingence
}

/// Identifiant unique d'une tâche
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Une tâche planifiable sur Mars
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarsTask {
    pub id: TaskId,
    pub name: String,
    pub category: TaskCategory,
    pub priority: u16,       // 0-100, calculé par l'IA
    pub deadline: Option<DateTime<Utc>>,
    pub estimated_duration_secs: u32,
    pub dependencies: Vec<TaskId>,
    pub required_resources: Vec<ResourceRequirement>,
    pub executing_node: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub scientific_value: f64, // Score scientifique 0.0-1.0
}

/// Exigence de ressource pour une tâche
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub resource_type: String,
    pub amount: f64,
    pub unit: String,
    pub critical: bool, // Si false, la tâche peut être réduite
}

// ─── Erreurs ───

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Tâche introuvable: {0}")]
    TaskNotFound(TaskId),

    #[error("Cycle de dépendances détecté dans: {0}")]
    DependencyCycle(String),

    #[error("Ressources insuffisantes pour: {0}")]
    InsufficientResources(String),

    #[error("Deadline dépassée: {0}")]
    DeadlineExceeded(TaskId),

    #[error("Conflit non résolu: {0}")]
    UnresolvedConflict(String),
}

pub type SchedulerResult<T> = Result<T, SchedulerError>;

// ─── Scheduler Principal ───

/// Scheduler autonome embarqué
pub struct AutonomousScheduler {
    tasks: HashMap<TaskId, MarsTask>,
    schedule: Vec<TaskId>,
    prioritizer: task_prioritizer::TaskPrioritizer,
    resolver: conflict_resolver::ConflictResolver,
    contingency: contingency_planner::ContingencyPlanner,
}

impl AutonomousScheduler {
    /// Crée un nouveau scheduler autonome
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            schedule: Vec::new(),
            prioritizer: task_prioritizer::TaskPrioritizer::new(),
            resolver: conflict_resolver::ConflictResolver::new(),
            contingency: contingency_planner::ContingencyPlanner::new(),
        }
    }

    /// Ajoute une nouvelle tâche au scheduler
    pub fn add_task(&mut self, task: MarsTask) -> SchedulerResult<&TaskId> {
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        info!("Task added: {} (category={:?}, priority={})", 
              id, self.tasks[&id].category, self.tasks[&id].priority);
        Ok(&id)
    }

    /// Supprime une tâche du scheduler
    pub fn remove_task(&mut self, task_id: &TaskId) -> SchedulerResult<MarsTask> {
        self.tasks.remove(task_id)
            .ok_or(SchedulerError::TaskNotFound(task_id.clone()))
    }

    /// Exécute la priorisation complète de toutes les tâches
    pub fn prioritize_all(&mut self) -> Vec<TaskId> {
        let tasks: Vec<&MarsTask> = self.tasks.values().collect();
        let prioritized = self.prioritizer.prioritize(&tasks);

        self.schedule = prioritized.iter().map(|t| t.id.clone()).collect();
        info!("Prioritized {} tasks", self.schedule.len());
        self.schedule.clone()
    }

    /// Résout les conflits dans le planning
    pub fn resolve_conflicts(&mut self) -> Vec<conflict_resolver::ConflictResolution> {
        let tasks: Vec<&MarsTask> = self.tasks.values()
            .filter(|t| t.status == TaskStatus::Scheduled || t.status == TaskStatus::InProgress)
            .collect();

        self.resolver.resolve(&tasks)
    }

    /// Génère un plan de contingence pour une tâche défaillante
    pub fn generate_contingency(&self, failed_task: &TaskId) -> Option<contingency_planner::ContingencyPlan> {
        let task = self.tasks.get(failed_task)?;
        self.contingency.plan(task, &self.tasks)
    }

    /// Produit le planning ordonné complet
    pub fn full_schedule(&self) -> Vec<&MarsTask> {
        self.schedule
            .iter()
            .filter_map(|id| self.tasks.get(id))
            .collect()
    }

    /// Statistiques du scheduler
    pub fn statistics(&self) -> SchedulerStats {
        let by_status: HashMap<TaskStatus, usize> = self.tasks.values()
            .fold(HashMap::new(), |mut acc, t| {
                *acc.entry(t.status.clone()).or_insert(0) += 1;
                acc
            });

        SchedulerStats {
            total_tasks: self.tasks.len(),
            completed: by_status.get(&TaskStatus::Completed).copied().unwrap_or(0),
            in_progress: by_status.get(&TaskStatus::InProgress).copied().unwrap_or(0),
            failed: by_status.get(&TaskStatus::Failed).copied().unwrap_or(0),
            queued: by_status.get(&TaskStatus::Queued).copied().unwrap_or(0),
            average_priority: if self.tasks.is_empty() {
                0.0
            } else {
                self.tasks.values().map(|t| t.priority as f64).sum::<f64>() / self.tasks.len() as f64
            },
        }
    }
}

/// Statistiques du scheduler
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: usize,
    pub completed: usize,
    pub in_progress: usize,
    pub failed: usize,
    pub queued: usize,
    pub average_priority: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(id: &str, category: TaskCategory, priority: u16) -> MarsTask {
        MarsTask {
            id: TaskId(id.into()),
            name: format!("Task {}", id),
            category,
            priority,
            deadline: None,
            estimated_duration_secs: 300,
            dependencies: vec![],
            required_resources: vec![],
            executing_node: None,
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            scientific_value: 0.8,
        }
    }

    #[test]
    fn test_scheduler_basic_flow() {
        let mut scheduler = AutonomousScheduler::new();

        let t1 = sample_task("task-1", TaskCategory::Science, 80);
        let t2 = sample_task("task-2", TaskCategory::Navigation, 60);
        let t3 = sample_task("task-3", TaskCategory::Emergency, 100);

        scheduler.add_task(t1).unwrap();
        scheduler.add_task(t2).unwrap();
        scheduler.add_task(t3).unwrap();

        let schedule = scheduler.prioritize_all();
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0], TaskId("task-3".into())); // Emergency en premier
    }

    #[test]
    fn test_scheduler_statistics() {
        let mut scheduler = AutonomousScheduler::new();

        for i in 0..5 {
            let task = MarsTask {
                status: if i < 3 { TaskStatus::Completed } else { TaskStatus::Queued },
                priority: 50 + i,
                ..sample_task(&format!("task-{}", i), TaskCategory::Science, 50 + i)
            };
            scheduler.add_task(task).unwrap();
        }

        let stats = scheduler.statistics();
        assert_eq!(stats.total_tasks, 5);
        assert_eq!(stats.completed, 3);
        assert_eq!(stats.queued, 2);
    }
}
