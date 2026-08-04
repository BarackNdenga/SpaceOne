//! # Prioriseur de Tâches
//!
//! Classe les tâches martiennes par ordre de priorité en utilisant
//! un modèle multi-critères qui combine :
//! - Urgence temporelle (proximité de la deadline)
//! - Valeur scientifique (impact de la mission)
//! - Catégorie critique (emergency > life_support > science)
//! - Dépendances (bloquées vs libres)

use crate::{MarsTask, TaskCategory, TaskStatus, TaskId};
use std::cmp::Ordering;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Poids relatifs des critères de priorisation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityWeights {
    pub urgency_weight: f64,      // Poids de l'urgence temporelle
    pub science_weight: f64,      // Poids de la valeur scientifique
    pub category_weight: f64,     // Poids de la catégorie critique
    pub dependency_weight: f64,   // Poids des dépendances
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            urgency_weight: 0.30,
            science_weight: 0.25,
            category_weight: 0.30,
            dependency_weight: 0.15,
        }
    }
}

/// Score de priorité calculé pour une tâche
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityScore {
    pub task_id: TaskId,
    pub total_score: f64,
    pub urgency_score: f64,
    pub science_score: f64,
    pub category_score: f64,
    pub dependency_score: f64,
}

/// Wrapper pour le tri (BinaryHeap = max-heap en Rust)
struct PriorityWrapper(PriorityScore);

impl PartialEq for PriorityWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_score == other.0.total_score
    }
}

impl Eq for PriorityWrapper {}

impl PartialOrd for PriorityWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap est un max-heap, donc on inverse pour avoir les hautes priorités en premier
        other.0.total_score.partial_cmp(&self.0.total_score).unwrap_or(Ordering::Equal)
    }
}

/// Le prioriseur de tâches
pub struct TaskPrioritizer {
    weights: PriorityWeights,
}

impl TaskPrioritizer {
    /// Crée un prioriseur avec les poids par défaut
    pub fn new() -> Self {
        Self {
            weights: PriorityWeights::default(),
        }
    }

    /// Crée un prioriseur avec des poids personnalisés
    pub fn with_weights(weights: PriorityWeights) -> Self {
        Self { weights }
    }

    /// Calcule le score de priorité pour une tâche unique
    pub fn score_task(&self, task: &MarsTask) -> PriorityScore {
        let urgency = self.compute_urgency(task);
        let science = self.compute_science_value(task);
        let category = self.compute_category_priority(task);
        let dependency = self.compute_dependency_readiness(task);

        let total = self.weights.urgency_weight * urgency
            + self.weights.science_weight * science
            + self.weights.category_weight * category
            + self.weights.dependency_weight * dependency;

        PriorityScore {
            task_id: task.id.clone(),
            total_score: total,
            urgency_score: urgency,
            science_score: science,
            category_score: category,
            dependency_score: dependency,
        }
    }

    /// Priorise une liste de tâches et retourne l'ordre trié
    pub fn prioritize(&self, tasks: &[&MarsTask]) -> Vec<&MarsTask> {
        let mut scored: Vec<(PriorityScore, &&MarsTask)> = tasks
            .iter()
            .map(|t| (self.score_task(t), t))
            .collect();

        scored.sort_by(|a, b| {
            b.0.total_score
                .partial_cmp(&a.0.total_score)
                .unwrap_or(Ordering::Equal)
        });

        scored.into_iter().map(|(_, t)| *t).collect()
    }

    /// Calcule le score d'urgence (0.0-1.0)
    fn compute_urgency(&self, task: &MarsTask) -> f64 {
        match &task.deadline {
            None => 0.5, // Pas de deadline = urgence moyenne
            Some(deadline) => {
                let now = Utc::now();
                if *deadline <= now {
                    return 1.0; // Déjà en retard = urgence maximale
                }
                let remaining = deadline.signed_duration_since(now);
                let total_minutes = remaining.num_minutes().max(1) as f64;
                // Plus le temps restant est court, plus l'urgence est haute
                (1.0 - (total_minutes / (24.0 * 60.0)).min(1.0)).max(0.0)
            }
        }
    }

    /// Calcule le score de valeur scientifique (0.0-1.0)
    fn compute_science_value(&self, task: &MarsTask) -> f64 {
        // Le scientific_value est déjà normalisé 0.0-1.0
        task.scientific_value
    }

    /// Calcule le score de priorité par catégorie (0.0-1.0)
    fn compute_category_priority(&self, task: &MarsTask) -> f64 {
        match task.category {
            TaskCategory::Emergency => 1.0,
            TaskCategory::LifeSupport => 0.9,
            TaskCategory::Communication => 0.6,
            TaskCategory::Maintenance => 0.5,
            TaskCategory::Science => 0.7,
            TaskCategory::Navigation => 0.4,
        }
    }

    /// Calcule le score de readiness des dépendances (0.0-1.0)
    fn compute_dependency_readiness(&self, task: &MarsTask) -> f64 {
        if task.dependencies.is_empty() {
            return 1.0; // Pas de dépendances = prêt
        }

        // Simule la readiness (dans un cas réel, on vérifierait le statut des tâches parentes)
        // Ici on assume que toutes les dépendances sont satisfaites
        // pour le scoring; le scheduler principal filtrera les tâches bloquées
        match task.status {
            TaskStatus::Queued => 0.5,
            TaskStatus::Scheduled => 0.7,
            TaskStatus::InProgress => 0.3, // En cours mais dépendances non résolues
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_task(id: &str, category: TaskCategory, deadline: Option<DateTime<Utc>>, value: f64) -> MarsTask {
        MarsTask {
            id: TaskId(id.into()),
            name: id.into(),
            category,
            priority: 50,
            deadline,
            estimated_duration_secs: 300,
            dependencies: vec![],
            required_resources: vec![],
            executing_node: None,
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            scientific_value: value,
        }
    }

    #[test]
    fn test_emergency_always_first() {
        let prioritizer = TaskPrioritizer::new();

        let tasks = vec![
            make_task("science", TaskCategory::Science, None, 0.9),
            make_task("emergency", TaskCategory::Emergency, None, 0.1),
            make_task("nav", TaskCategory::Navigation, None, 0.5),
        ];

        let references: Vec<&MarsTask> = tasks.iter().collect();
        let result = prioritizer.prioritize(&references);

        assert_eq!(result[0].id.0, "emergency");
    }

    #[test]
    fn test_urgency_with_deadline() {
        let prioritizer = TaskPrioritizer::new();

        let urgent = make_task(
            "urgent",
            TaskCategory::Science,
            Some(Utc::now() + Duration::minutes(5)),
            0.8,
        );
        let relaxed = make_task(
            "relaxed",
            TaskCategory::Science,
            Some(Utc::now() + Duration::hours(24)),
            0.8,
        );

        let score_urgent = prioritizer.score_task(&urgent);
        let score_relaxed = prioritizer.score_task(&relaxed);

        assert!(score_urgent.urgency_score > score_relaxed.urgency_score);
    }

    #[test]
    fn test_custom_weights() {
        let weights = PriorityWeights {
            urgency_weight: 0.5,
            science_weight: 0.1,
            category_weight: 0.3,
            dependency_weight: 0.1,
        };

        let prioritizer = TaskPrioritizer::with_weights(weights);
        let task = make_task("test", TaskCategory::Emergency, None, 0.5);
        let score = prioritizer.score_task(&task);

        // Category emergency = 1.0, donc category_score contribution = 0.3 * 1.0 = 0.3
        assert!((score.category_score - 1.0).abs() < 0.01);
    }
}
