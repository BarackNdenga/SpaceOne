//! # Types de Messages MAP
//!
//! Définit les quatre catégories de messages du protocole MAP :
//! - **Query** : Demande d'information sur l'état, la position ou les ressources
//! - **Command** : Ordre avec priorité, deadline et contexte
//! - **Response** : Réponse à un Query ou confirmation d'un Command
//! - **Negotiation** : Proposition et résolution de conflits de ressources

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sha3::{Sha3_256, Digest};
use crate::{NodeId, AgencyId, PlatformType, NodeStatus};

// ─── Identifiant de Message ───

/// Identifiant unique d'un message MAP (hash SHA3-256)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

impl MessageId {
    /// Génère un nouvel identifiant unique à partir du contenu
    pub fn generate(content: &[u8]) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(content);
        MessageId(hex::encode(hasher.finalize()))
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0[..16])
    }
}

// ─── Priorité ───

/// Niveau de priorité d'un message (0 = lowest, 7 = critical)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Background = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Urgent = 4,
    Critical = 5,
    Emergency = 6,
    SafetyCritical = 7,
}

impl Priority {
    /// Vérifie si cette priorité est suffisante pour l'opération demandée
    pub fn is_sufficient_for(&self, required: Priority) -> bool {
        (*self as u8) >= (required as u8)
    }
}

// ─── Query Messages ───

/// Requête d'information vers un pair MAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMessage {
    pub id: MessageId,
    pub from: NodeId,
    pub from_agency: AgencyId,
    pub to: Option<NodeId>, // None = broadcast
    pub timestamp: DateTime<Utc>,
    pub priority: Priority,
    pub query_type: QueryType,
    pub ttl_seconds: u32, // Time-to-live
}

/// Types de requêtes supportées
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    /// Demande l'état actuel d'un noeud
    StatusRequest,
    /// Demande les capacités disponibles
    CapabilitiesRequest,
    /// Demande la position relative
    PositionRequest,
    /// Demande les ressources disponibles
    ResourceRequest,
    /// Demande le niveau de charge actuel
    LoadRequest,
    /// Demande la disponibilité pour une tâche
    AvailabilityRequest {
        task: String,
        duration_seconds: u32,
    },
    /// Demande d'inventaire des ressources
    InventoryRequest {
        resource_types: Vec<String>,
    },
}

// ─── Command Messages ───

/// Commande envoyée à un pair MAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    pub id: MessageId,
    pub from: NodeId,
    pub from_agency: AgencyId,
    pub to: NodeId,
    pub timestamp: DateTime<Utc>,
    pub priority: Priority,
    pub command_type: CommandType,
    pub deadline: DateTime<Utc>,
    pub context: CommandContext,
    pub requires_ack: bool,
}

/// Types de commandes supportées
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    /// Déplacement vers une position
    NavigateTo {
        target: CoordTarget,
        path_priority: Priority,
    },
    /// Collecte d'échantillon
    SampleCollection {
        target_location: CoordTarget,
        sample_type: String,
        max_depth_cm: f64,
    },
    /// Transmission de données
    DataTransfer {
        target_node: NodeId,
        data_type: String,
        size_bytes: u64,
        priority: Priority,
    },
    /// Activation d'un système
    ActivateSystem {
        system_name: String,
        duration_seconds: Option<u32>,
    },
    /// Désactivation d'un système
    DeactivateSystem {
        system_name: String,
        reason: String,
    },
    /// Entrée en mode safe
    EnterSafeMode {
        reason: String,
        severity: u8, // 0-10
    },
    /// Requête de coopération pour une tâche jointe
    CooperativeTask {
        task_id: String,
        description: String,
        required_capabilities: Vec<String>,
        deadline: DateTime<Utc>,
    },
    /// Réallocation de ressources
    ResourceReallocation {
        resource: String,
        from_node: NodeId,
        to_node: NodeId,
        quantity: f64,
    },
}

/// Cible de coordonnées martiennes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordTarget {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub reference_frame: String, // "mars_body_fixed" ou "relative"
}

/// Contexte d'une commande
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContext {
    pub mission_phase: String,
    pub scientific_objective: Option<String>,
    pub safety_constraints: Vec<String>,
    pub energy_budget_wh: Option<f64>,
    pub communication_window: Option<DateTime<Utc>>,
}

// ─── Response Messages ───

/// Réponse à un Query ou confirmation d'un Command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub id: MessageId,
    pub in_response_to: MessageId,
    pub from: NodeId,
    pub from_agency: AgencyId,
    pub to: NodeId,
    pub timestamp: DateTime<Utc>,
    pub status: ResponseStatus,
    pub response_data: ResponseData,
}

/// Statut d'une réponse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    PartialSuccess { details: String },
    Rejected { reason: String },
    Busy,
    InsufficientResources,
    CapabilityMismatch,
    PriorityConflict,
    Timeout,
    Error { code: u16, message: String },
}

/// Données de réponse (variable selon le type de requête)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseData {
    /// État courant du noeud
    StatusResponse {
        status: NodeStatus,
        platform: PlatformType,
        capabilities: Vec<String>,
        battery_percent: f64,
        temperature_c: f64,
        uptime_seconds: u64,
    },
    /// Liste des capacités disponibles
    CapabilitiesResponse {
        capabilities: Vec<CapabilityDetail>,
    },
    /// Position relative
    PositionResponse {
        latitude: f64,
        longitude: f64,
        altitude: f64,
        confidence_m: f64,
        relative_to: Option<NodeId>,
    },
    /// Ressources disponibles
    ResourceResponse {
        resources: Vec<ResourceInfo>,
    },
    /// Confirmation d'exécution
    ExecutionConfirm {
        estimated_completion: DateTime<Utc>,
        estimated_energy_wh: f64,
    },
    /// Rejet de commande
    Rejection {
        reason: String,
        suggested_alternative: Option<String>,
    },
    /// Données brutes (inventaire, télémétrie)
    RawData {
        data_type: String,
        payload: Vec<u8>,
    },
}

/// Détail d'une capacité
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDetail {
    pub name: String,
    pub status: String, // "ready", "in_use", "maintenance"
    pub availability: f64, // 0.0 - 1.0
    pub constraints: Vec<String>,
}

/// Informations sur une ressource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub resource_type: String,
    pub available: f64,
    pub total: f64,
    pub unit: String,
    pub depletion_rate: Option<f64>,
}

// ─── Negotiation Messages ───

/// Message de négociation pour la résolution de conflits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationMessage {
    pub id: MessageId,
    pub from: NodeId,
    pub from_agency: AgencyId,
    pub participants: Vec<NodeId>,
    pub timestamp: DateTime<Utc>,
    pub negotiation_type: NegotiationType,
    pub proposal: NegotiationProposal,
    pub round: u32,
    pub max_rounds: u32,
}

/// Types de négociation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NegotiationType {
    /// Conflit d'accès à une ressource partagée
    ResourceConflict {
        resource: String,
        contested_by: Vec<NodeId>,
    },
    /// Attribution d'un chemin (évitement de collision)
    PathConflict {
        origin: NodeId,
        contested_path: Vec<CoordTarget>,
    },
    /// Partage de charge de travail
    WorkloadBalancing {
        task: String,
        total_work: f64,
    },
    /// Priorité d'accès au réseau de communication
    CommsBandwidth {
        total_bandwidth_kbps: f64,
        requested_by: Vec<(NodeId, f64)>,
    },
    /// Planification de window d'observation
    ObservationWindow {
        target: String,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    },
}

/// Proposition de négociation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationProposal {
    pub proposer: NodeId,
    pub allocation: Vec<NodeAllocation>,
    pub rationale: String,
}

/// Allocation proposée pour un noeud dans la négociation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAllocation {
    pub node_id: NodeId,
    pub share: f64, // Proportion 0.0 - 1.0
    pub time_slot: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub constraints: Vec<String>,
}

/// Résultat final d'une négociation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationResult {
    pub negotiation_id: MessageId,
    pub status: NegotiationStatus,
    pub final_allocation: Vec<NodeAllocation>,
    pub consensus_rounds: u32,
    pub agreements: Vec<NodeId>,
    pub dissenters: Vec<NodeId>,
}

/// Statut d'une négociation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NegotiationStatus {
    InProgress,
    Completed,
    Failed,
    Timeout,
    Overridden { by: NodeId, reason: String },
}

// ─── Enveloppe MAP Unifiée ───

/// Enveloppe principale de tout message MAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMessage {
    pub version: u8,
    pub message_id: MessageId,
    pub timestamp: DateTime<Utc>,
    pub sender: NodeId,
    pub sender_agency: AgencyId,
    pub recipients: Vec<NodeId>,
    pub ttl_seconds: u32,
    pub hop_count: u32,
    pub payload: MapPayload,
    pub signature: Option<Vec<u8>>,
}

/// Payload du message MAP (l'un des quatre types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapPayload {
    Query(QueryMessage),
    Command(CommandMessage),
    Response(ResponseMessage),
    Negotiation(NegotiationMessage),
    NegotiationResult(NegotiationResult),
}

impl MapMessage {
    /// Crée un nouveau message MAP avec signature SHA3
    pub fn new(sender: NodeId, sender_agency: AgencyId, recipients: Vec<NodeId>, payload: MapPayload) -> Self {
        let content = serde_json::to_vec(&(&sender, &sender_agency, &recipients, &payload)).unwrap_or_default();
        let message_id = MessageId::generate(&content);

        MapMessage {
            version: 1,
            message_id,
            timestamp: Utc::now(),
            sender,
            sender_agency,
            recipients,
            ttl_seconds: 3600,
            hop_count: 0,
            payload,
            signature: None,
        }
    }

    /// Incrémente le hop count (pour le TTL DTN)
    pub fn increment_hop(&mut self) -> bool {
        self.hop_count += 1;
        if (self.hop_count as u32) >= self.ttl_seconds / 60 {
            return false; // TTL expiré
        }
        true
    }

    /// Vérifie l'intégrité du message
    pub fn verify_integrity(&self) -> bool {
        let content = serde_json::to_vec(&(&self.sender, &self.sender_agency, &self.recipients, &self.payload));
        match content {
            Ok(data) => {
                let expected = MessageId::generate(&data);
                expected == self.message_id
            }
            Err(_) => false,
        }
    }

    /// Type de payload
    pub fn payload_type(&self) -> &'static str {
        match &self.payload {
            MapPayload::Query(_) => "Query",
            MapPayload::Command(_) => "Command",
            MapPayload::Response(_) => "Response",
            MapPayload::Negotiation(_) => "Negotiation",
            MapPayload::NegotiationResult(_) => "NegotiationResult",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeId, AgencyId};

    #[test]
    fn test_message_id_generation() {
        let content = b"test content";
        let id1 = MessageId::generate(content);
        let id2 = MessageId::generate(content);
        assert_eq!(id1, id2); // Déterministe

        let id3 = MessageId::generate(b"different");
        assert_ne!(id1, id3); // Différent pour contenu différent
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::Normal);
        assert!(Priority::SafetyCritical >= Priority::Emergency);
        assert!(Priority::Background < Priority::Low);
        assert!(Priority::SafetyCritical.is_sufficient_for(Priority::Critical));
        assert!(!Priority::Normal.is_sufficient_for(Priority::High));
    }

    #[test]
    fn test_map_message_creation() {
        let msg = MapMessage::new(
            NodeId("rover-01".into()),
            AgencyId::Nasa,
            vec![NodeId("habitat-01".into())],
            MapPayload::Query(QueryMessage {
                id: MessageId::generate(b"test"),
                from: NodeId("rover-01".into()),
                from_agency: AgencyId::Nasa,
                to: Some(NodeId("habitat-01".into())),
                timestamp: Utc::now(),
                priority: Priority::Normal,
                query_type: QueryType::StatusRequest,
                ttl_seconds: 3600,
            }),
        );

        assert_eq!(msg.payload_type(), "Query");
        assert!(msg.verify_integrity());
        assert_eq!(msg.hop_count, 0);
    }

    #[test]
    fn test_hop_count_increment() {
        let mut msg = MapMessage::new(
            NodeId("rover-01".into()),
            AgencyId::Nasa,
            vec![],
            MapPayload::Query(QueryMessage {
                id: MessageId::generate(b"ttl-test"),
                from: NodeId("rover-01".into()),
                from_agency: AgencyId::Nasa,
                to: None,
                timestamp: Utc::now(),
                priority: Priority::Normal,
                query_type: QueryType::StatusRequest,
                ttl_seconds: 3600,
            }),
        );

        for _ in 0..59 {
            assert!(msg.increment_hop());
        }
        // Le 60ème hop devrait échouer (TTL = 3600s / 60 = 60 hops max)
        assert!(!msg.increment_hop());
    }
}
