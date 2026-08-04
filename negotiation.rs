//! # Négociation MAP — Résolution de Conflits et Allocation de Ressources
//!
//! Implémente le mécanisme de négociation distribué pour résoudre les conflits
//! entre agences et allouer les ressources partagées sur Mars.
//!
//! ## Algorithme
//!
//! La négociation suit un protocole en plusieurs tours :
//! 1. **Proposal** : Un initiateur propose une allocation
//! 2. **Feedback** : Chaque participant répond (accepte, contre-propose, refuse)
//! 3. **Iteration** : Si pas de consensus, nouvelle proposition ajustée
//! 4. **Resolution** : Consensus ou escalation à une autorité supérieure

use crate::{NodeId, AgencyId, MapResult, MapError, message_types::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ─── État de Négociation ───

/// État d'une négociation en cours
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationState {
    pub id: MessageId,
    pub negotiation_type: NegotiationType,
    pub participants: Vec<NodeId>,
    pub initiator: NodeId,
    pub current_round: u32,
    pub max_rounds: u32,
    pub proposals: Vec<NegotiationProposal>,
    pub responses: HashMap<NodeId, NegotiationResponse>,
    pub status: NegotiationStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Réponse d'un participant à une proposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationResponse {
    pub from: NodeId,
    pub round: u32,
    pub decision: NegotiationDecision,
    pub counter_proposal: Option<NegotiationProposal>,
    pub rationale: String,
}

/// Décision d'un participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NegotiationDecision {
    Accept,
    Reject { reason: String },
    CounterPropose,
    Abstain,
}

// ─── Moteur de Négociation ───

/// Moteur qui gère toutes les négociations actives
pub struct NegotiationEngine {
    active: Arc<RwLock<HashMap<MessageId, NegotiationState>>>,
    completed: Arc<RwLock<Vec<NegotiationResult>>>,
    default_max_rounds: u32,
}

impl NegotiationEngine {
    /// Crée un nouveau moteur de négociation
    pub fn new(max_rounds: u32) -> Self {
        Self {
            active: Arc::new(RwLock::new(HashMap::new())),
            completed: Arc::new(RwLock::new(Vec::new())),
            default_max_rounds: max_rounds,
        }
    }

    /// Lance une nouvelle négociation
    pub async fn start_negotiation(
        &self,
        initiator: &NodeId,
        participants: Vec<NodeId>,
        negotiation_type: NegotiationType,
        initial_proposal: NegotiationProposal,
    ) -> MapResult<MessageId> {
        let content = serde_json::to_vec(&(initiator, &negotiation_type))
            .unwrap_or_default();
        let id = MessageId::generate(&content);

        let state = NegotiationState {
            id: id.clone(),
            negotiation_type,
            participants: participants.clone(),
            initiator: initiator.clone(),
            current_round: 0,
            max_rounds: self.default_max_rounds,
            proposals: vec![initial_proposal],
            responses: HashMap::new(),
            status: NegotiationStatus::InProgress,
            created_at: chrono::Utc::now(),
        };

        let mut active = self.active.write().await;
        active.insert(id.clone(), state);

        info!(
            "Started negotiation {} with {} participants",
            id,
            participants.len()
        );

        Ok(id)
    }

    /// Enregistre la réponse d'un participant
    pub async fn submit_response(
        &self,
        negotiation_id: &MessageId,
        response: NegotiationResponse,
    ) -> MapResult<Option<NegotiationResult>> {
        let mut active = self.active.write().await;
        let state = active
            .get_mut(negotiation_id)
            .ok_or_else(|| MapError::NegotiationTimeout(0))?;

        // Enregistrer la réponse
        state.responses.insert(response.from.clone(), response);

        // Vérifier si tous les participants ont répondu
        let all_responded = state
            .participants
            .iter()
            .all(|p| state.responses.contains_key(p));

        if all_responded {
            // Analyser les réponses
            let result = self.analyze_responses(state)?;
            state.status = result.status.clone();

            // Enregistrer comme complétée
            let mut completed = self.completed.write().await;
            completed.push(result.clone());

            // Nettoyer
            active.remove(negotiation_id);

            info!("Negotiation {} completed: {:?}", negotiation_id, result.status);
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Analyse les réponses et détermine le résultat
    fn analyze_responses(&self, state: &NegotiationState) -> MapResult<NegotiationResult> {
        let accepts: Vec<&NodeId> = state
            .responses
            .iter()
            .filter(|(_, r)| matches!(r.decision, NegotiationDecision::Accept))
            .map(|(nid, _)| nid)
            .collect();

        let rejects: Vec<(&NodeId, &str)> = state
            .responses
            .iter()
            .filter(|(_, r)| matches!(r.decision, NegotiationDecision::Reject { .. }))
            .map(|(nid, r)| {
                (
                    nid,
                    match &r.decision {
                        NegotiationDecision::Reject { reason } => reason.as_str(),
                        _ => "",
                    },
                )
            })
            .collect();

        let counter_proposals: Vec<&NodeId> = state
            .responses
            .iter()
            .filter(|(_, r)| matches!(r.decision, NegotiationDecision::CounterPropose))
            .map(|(nid, _)| nid)
            .collect();

        let consensus_threshold = (state.participants.len() as f64 * 0.67).ceil() as usize;

        if accepts.len() >= consensus_threshold {
            // Consensus atteint — utiliser la dernière proposition
            let final_allocation = state
                .proposals
                .last()
                .map(|p| p.allocation.clone())
                .unwrap_or_default();

            Ok(NegotiationResult {
                negotiation_id: state.id.clone(),
                status: NegotiationStatus::Completed,
                final_allocation,
                consensus_rounds: state.current_round,
                agreements: accepts.into_iter().cloned().collect(),
                dissenters: rejects.iter().map(|(nid, _)| (*nid).clone()).collect(),
            })
        } else if !counter_proposals.is_empty() && state.current_round < state.max_rounds {
            // Besoin d'une nouvelle proposition — échec temporaire
            Ok(NegotiationResult {
                negotiation_id: state.id.clone(),
                status: NegotiationStatus::InProgress,
                final_allocation: vec![],
                consensus_rounds: state.current_round,
                agreements: vec![],
                dissenters: rejects
                    .iter()
                    .chain(counter_proposals.iter())
                    .map(|(nid, _)| (*nid).clone())
                    .collect(),
            })
        } else {
            // Échec — pas de consensus
            warn!(
                "Negotiation {} failed after {} rounds ({} accepts, {} rejects)",
                state.id, state.current_round, accepts.len(), rejects.len()
            );

            Ok(NegotiationResult {
                negotiation_id: state.id.clone(),
                status: NegotiationStatus::Failed,
                final_allocation: vec![],
                consensus_rounds: state.current_round,
                agreements: accepts.into_iter().cloned().collect(),
                dissenters: rejects
                    .iter()
                    .map(|(nid, _)| (*nid).clone())
                    .collect(),
            })
        }
    }

    /// Incrémente le round de négociation
    pub async fn next_round(
        &self,
        negotiation_id: &MessageId,
        new_proposal: NegotiationProposal,
    ) -> MapResult<()> {
        let mut active = self.active.write().await;
        let state = active
            .get_mut(negotiation_id)
            .ok_or_else(|| MapError::NegotiationTimeout(0))?;

        state.current_round += 1;
        state.responses.clear(); // Réinitialiser les réponses
        state.proposals.push(new_proposal);

        if state.current_round >= state.max_rounds {
            state.status = NegotiationStatus::Timeout;
        }

        Ok(())
    }

    /// Statistiques des négociations
    pub async fn statistics(&self) -> NegotiationStats {
        let active = self.active.read().await;
        let completed = self.completed.read().await;

        let total_completed = completed.len();
        let successful = completed
            .iter()
            .filter(|r| r.status == NegotiationStatus::Completed)
            .count();
        let failed = completed
            .iter()
            .filter(|r| r.status == NegotiationStatus::Failed)
            .count();
        let timeout = completed
            .iter()
            .filter(|r| r.status == NegotiationStatus::Timeout)
            .count();

        NegotiationStats {
            active_count: active.len(),
            total_completed,
            successful,
            failed,
            timeout,
            success_rate: if total_completed > 0 {
                (successful as f64) / (total_completed as f64)
            } else {
                1.0
            },
        }
    }

    /// Liste des négociations actives
    pub async fn active_negotiations(&self) -> Vec<MessageId> {
        let active = self.active.read().await;
        active.keys().cloned().collect()
    }
}

/// Statistiques des négociations
#[derive(Debug, Clone, Serialize)]
pub struct NegotiationStats {
    pub active_count: usize,
    pub total_completed: usize,
    pub successful: usize,
    pub failed: usize,
    pub timeout: usize,
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_negotiation() {
        let engine = NegotiationEngine::new(3);

        let initiator = NodeId("rover-nasa".into());
        let participants = vec![
            NodeId("habitat-esa".into()),
            NodeId("orbiter-spacex".into()),
        ];

        let proposal = NegotiationProposal {
            proposer: initiator.clone(),
            allocation: vec![
                NodeAllocation {
                    node_id: NodeId("rover-nasa".into()),
                    share: 0.5,
                    time_slot: None,
                    constraints: vec!["priority_path".into()],
                },
                NodeAllocation {
                    node_id: NodeId("habitat-esa".into()),
                    share: 0.3,
                    time_slot: None,
                    constraints: vec![],
                },
                NodeAllocation {
                    node_id: NodeId("orbiter-spacex".into()),
                    share: 0.2,
                    time_slot: None,
                    constraints: vec![],
                },
            ],
            rationale: "Resource sharing for coordinated exploration".into(),
        };

        let neg_id = engine
            .start_negotiation(
                &initiator,
                participants.clone(),
                NegotiationType::ResourceConflict {
                    resource: "comms_bandwidth".into(),
                    contested_by: participants.clone(),
                },
                proposal,
            )
            .await
            .unwrap();

        // Tous acceptent
        for node_id in &participants {
            let response = NegotiationResponse {
                from: node_id.clone(),
                round: 0,
                decision: NegotiationDecision::Accept,
                counter_proposal: None,
                rationale: "Acceptable allocation".into(),
            };
            let result = engine.submit_response(&neg_id, response).await;
            assert!(result.is_ok());
        }

        // L'initiateur accepte aussi
        let response = NegotiationResponse {
            from: initiator.clone(),
            round: 0,
            decision: NegotiationDecision::Accept,
            counter_proposal: None,
            rationale: "Initiator accepts own proposal".into(),
        };
        let result = engine.submit_response(&neg_id, response).await.unwrap();

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.status, NegotiationStatus::Completed);
        assert_eq!(result.agreements.len(), 3);
    }

    #[tokio::test]
    async fn test_negotiation_statistics() {
        let engine = NegotiationEngine::new(3);

        let initiator = NodeId("node-1".into());
        let participants = vec![NodeId("node-2".into())];

        let proposal = NegotiationProposal {
            proposer: initiator.clone(),
            allocation: vec![
                NodeAllocation {
                    node_id: NodeId("node-1".into()),
                    share: 0.5,
                    time_slot: None,
                    constraints: vec![],
                },
                NodeAllocation {
                    node_id: NodeId("node-2".into()),
                    share: 0.5,
                    time_slot: None,
                    constraints: vec![],
                },
            ],
            rationale: "test".into(),
        };

        let neg_id = engine
            .start_negotiation(&initiator, participants.clone(), NegotiationType::WorkloadBalancing {
                task: "test".into(),
                total_work: 100.0,
            }, proposal)
            .await
            .unwrap();

        // Accept
        for node in &[initiator.clone(), participants[0].clone()] {
            engine.submit_response(&neg_id, NegotiationResponse {
                from: node.clone(),
                round: 0,
                decision: NegotiationDecision::Accept,
                counter_proposal: None,
                rationale: "OK".into(),
            }).await.unwrap();
        }

        let stats = engine.statistics().await;
        assert_eq!(stats.total_completed, 1);
        assert_eq!(stats.successful, 1);
        assert_eq!(stats.active_count, 0);
    }
}
