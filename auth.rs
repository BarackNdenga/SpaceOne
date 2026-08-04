//! # Authentification & Autorisation MAP
//!
//! Système d'authentification inter-agences basé sur des certificats
//! distribués et un contrôle d'accès par capacités (capability-based).
//!
//! Chaque entité Mars possède un certificat d'identité signé par son
//! agence mère, et les permissions sont gérées via un modèle capability-based
//! plutôt que rôle-based (plus adapté aux environnements distribués).

use crate::{NodeId, AgencyId, MapError, MapResult};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;

// ─── Identité ───

/// Certificat d'identité d'une entité Mars
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarsIdentity {
    pub node_id: NodeId,
    pub agency: AgencyId,
    pub public_key: Vec<u8>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub mission: String,
    pub platform: String,
}

impl MarsIdentity {
    /// Génère le fingerprint du certificat (SHA3-256 de la clé publique)
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.public_key);
        hex::encode(hasher.finalize())
    }

    /// Vérifie si le certificat est encore valide
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.issued_at && now <= self.expires_at
    }

    /// Vérifie si le certificat a expiré
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Calcule les jours restants avant expiration
    pub fn days_until_expiry(&self) -> i64 {
        let delta = self.expires_at.signed_duration_since(Utc::now());
        delta.num_days()
    }
}

// ─── Capabilities ───

/// Une capacité (permission) accordée à une entité
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub scope: CapabilityScope,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Portée d'une capacité
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityScope {
    /// Applicables à tout le réseau Mars
    Global,
    /// Limitées à une agence spécifique
    Agency(AgencyId),
    /// Limitées à un noeud ou groupe de noeuds
    Nodes(Vec<NodeId>),
    /// Limitées à un type de ressource
    Resource(String),
}

/// Erreurs d'authentification
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Certificat expiré: {0}")]
    CertificateExpired(NodeId),

    #[error("Certificat non reconnu: {0}")]
    UnknownCertificate(String),

    #[error("Capacité insuffisante: required={required}, provided={provided}")]
    InsufficientCapability { required: String, provided: String },

    #[error("Agence non autorisée: {0}")]
    AgencyNotAuthorized(AgencyId),

    #[error("Signature invalide pour: {0}")]
    InvalidSignature(NodeId),

    #[error("Révocation active: {0}")]
    CertificateRevoked(NodeId),
}

// ─── Registre d'Authentification ───

/// Registre des identités et capacités validées
pub struct AuthRegistry {
    identities: Arc<RwLock<HashMap<NodeId, MarsIdentity>>>,
    capabilities: Arc<RwLock<HashMap<NodeId, HashSet<Capability>>>>,
    revoked: Arc<RwLock<HashSet<NodeId>>>,
    trusted_agencies: Arc<RwLock<HashSet<AgencyId>>>,
}

impl AuthRegistry {
    /// Crée un nouveau registre d'authentification
    pub fn new() -> Self {
        Self {
            identities: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            revoked: Arc::new(RwLock::new(HashSet::new())),
            trusted_agencies: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Enregistre une identité Mars
    pub async fn register_identity(&self, identity: MarsIdentity) -> MapResult<()> {
        if !identity.is_valid() {
            return Err(MapError::AuthFailed(format!(
                "Certificat invalide pour {}",
                identity.node_id
            )));
        }

        let mut identities = self.identities.write().await;
        identities.insert(identity.node_id.clone(), identity);
        Ok(())
    }

    /// Ajoute des capacités à un noeud
    pub async fn grant_capabilities(&self, node_id: &NodeId, caps: Vec<Capability>) {
        let mut capabilities = self.capabilities.write().await;
        capabilities
            .entry(node_id.clone())
            .or_default()
            .extend(caps);
    }

    /// Révoque une identité
    pub async fn revoke_identity(&self, node_id: &NodeId) {
        let mut revoked = self.revoked.write().await;
        revoked.insert(node_id.clone());

        let mut identities = self.identities.write().await;
        identities.remove(node_id);
    }

    /// Vérifie si un noeud est authentifié et non révoqué
    pub async fn authenticate(&self, node_id: &NodeId) -> Result<&MarsIdentity, AuthError> {
        // Vérifier la révocation
        let revoked = self.revoked.read().await;
        if revoked.contains(node_id) {
            return Err(AuthError::CertificateRevoked(node_id.clone()));
        }

        // Vérifier l'existence
        let identities = self.identities.read().await;
        let identity = identities.get(node_id).ok_or_else(|| {
            AuthError::UnknownCertificate(node_id.0.clone())
        })?;

        // Vérifier la validité temporelle
        if !identity.is_valid() {
            return Err(AuthError::CertificateExpired(node_id.clone()));
        }

        Ok(identity)
    }

    /// Vérifie si un noeud possède une capacité requise
    pub async fn check_capability(
        &self,
        node_id: &NodeId,
        required_capability: &str,
        scope: &CapabilityScope,
    ) -> bool {
        let capabilities = self.capabilities.read().await;
        let node_caps = match capabilities.get(node_id) {
            Some(caps) => caps,
            None => return false,
        };

        node_caps.iter().any(|cap| {
            cap.name == required_capability && cap_matches_scope(cap, scope)
        })
    }

    /// Nettoie les identités expirées et les capacités périmées
    pub async fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let mut count = 0;

        // Nettoyage des identités
        {
            let mut identities = self.identities.write().await;
            let expired: Vec<NodeId> = identities
                .iter()
                .filter(|(_, id)| id.expires_at < now)
                .map(|(nid, _)| nid.clone())
                .collect();
            count += expired.len();
            for nid in expired {
                identities.remove(&nid);
            }
        }

        // Nettoyage des capacités expirées
        {
            let mut capabilities = self.capabilities.write().await;
            for (_, caps) in capabilities.iter_mut() {
                caps.retain(|cap| {
                    cap.expires_at.map_or(true, |exp| exp > now)
                });
            }
        }

        count
    }

    /// Produit un rapport d'audit de sécurité
    pub async fn audit_report(&self) -> AuthAuditReport {
        let identities = self.identities.read().await;
        let capabilities = self.capabilities.read().await;
        let revoked = self.revoked.read().await;

        let now = Utc::now();

        AuthAuditReport {
            total_identities: identities.len(),
            total_capabilities: capabilities.values().map(|s| s.len()).sum(),
            revoked_count: revoked.len(),
            expiring_soon: identities
                .values()
                .filter(|id| id.days_until_expiry() < 30 && id.days_until_expiry() >= 0)
                .count(),
            expired: identities
                .values()
                .filter(|id| id.expires_at < now)
                .count(),
        }
    }
}

/// Vérifie si une capacité correspond à la portée demandée
fn cap_matches_scope(cap: &Capability, scope: &CapabilityScope) -> bool {
    use CapabilityScope::*;
    match (&cap.scope, scope) {
        (Global, _) => true,
        (_, Global) => false,
        (Agency(a1), Agency(a2)) => a1 == a2,
        (Nodes(nodes), Nodes(required)) => {
            nodes.iter().any(|n| required.contains(n))
        }
        (Resource(r1), Resource(r2)) => r1 == r2,
        _ => false,
    }
}

/// Rapport d'audit de sécurité
#[derive(Debug, Clone, Serialize)]
pub struct AuthAuditReport {
    pub total_identities: usize,
    pub total_capabilities: usize,
    pub revoked_count: usize,
    pub expiring_soon: usize,
    pub expired: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_identity(node_id: &str, agency: AgencyId) -> MarsIdentity {
        MarsIdentity {
            node_id: NodeId(node_id.into()),
            agency,
            public_key: vec![1, 2, 3, 4],
            issued_at: Utc::now() - chrono::Duration::hours(1),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            mission: "mars-2030".into(),
            platform: "rover".into(),
        }
    }

    #[tokio::test]
    async fn test_register_and_authenticate() {
        let registry = AuthRegistry::new();
        let identity = sample_identity("rover-01", AgencyId::Nasa);
        let fp = identity.fingerprint();

        registry.register_identity(identity).await.unwrap();

        let auth = registry.authenticate(&NodeId("rover-01".into())).await;
        assert!(auth.is_ok());
        assert_eq!(auth.unwrap().node_id.0, "rover-01");
    }

    #[tokio::test]
    async fn test_revoke_identity() {
        let registry = AuthRegistry::new();
        registry
            .register_identity(sample_identity("rover-01", AgencyId::Nasa))
            .await
            .unwrap();

        registry.revoke_identity(&NodeId("rover-01".into())).await;

        let auth = registry.authenticate(&NodeId("rover-01".into())).await;
        assert!(auth.is_err());
    }

    #[tokio::test]
    async fn test_capabilities_check() {
        let registry = AuthRegistry::new();
        let node_id = NodeId("rover-01".into());

        registry.grant_capabilities(
            &node_id,
            vec![
                Capability {
                    name: "navigate".into(),
                    scope: CapabilityScope::Global,
                    expires_at: None,
                },
                Capability {
                    name: "sample".into(),
                    scope: CapabilityScope::Agency(AgencyId::Nasa),
                    expires_at: None,
                },
            ],
        )
        .await;

        assert!(
            registry
                .check_capability(&node_id, "navigate", &CapabilityScope::Global)
                .await
        );
        assert!(
            registry
                .check_capability(&node_id, "sample", &CapabilityScope::Agency(AgencyId::Nasa))
                .await
        );
        assert!(
            !registry
                .check_capability(&node_id, "sample", &CapabilityScope::Agency(AgencyId::Esa))
                .await
        );
        assert!(
            !registry
                .check_capability(&node_id, "fly", &CapabilityScope::Global)
                .await
        );
    }
}
