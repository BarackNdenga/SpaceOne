//! # Access Control — RBAC et Capabilities
//!
//! Contrôle d'accès basé sur les rôles (RBAC) étendu avec
//! un modèle de capabilities pour les environnements distribués.

use crate::{SecurityError, SecurityResult, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};

/// Rôle dans le système
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    MissionCommander,
    FlightDirector,
    Scientist,
    Engineer,
    Operator,
    Observer,
    AutonomousSystem,
}

/// Permission (capability)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    pub action: String,
    pub resource: String,
    pub scope: PermissionScope,
}

/// Portée d'une permission
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionScope {
    Global,
    Node(String),
    Agency(String),
    System(String),
}

/// Rôle avec ses permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub role: Role,
    pub permissions: HashSet<Permission>,
    pub description: String,
    pub max_classification: crate::DataClassification,
}

/// Session authentifiée
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedSession {
    pub session_id: SessionId,
    pub principal: String,
    pub role: Role,
    pub capabilities: HashSet<Permission>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub node_id: String,
}

impl AuthenticatedSession {
    /// Vérifie si la session possède une permission donnée
    pub fn has_permission(&self, action: &str, resource: &str) -> bool {
        self.capabilities.iter().any(|p| {
            p.action == action && p.resource == resource
        })
    }

    /// Vérifie si la session est encore valide
    pub fn is_valid(&self) -> bool {
        Utc::now() <= self.expires_at
    }
}

/// Gestionnaire de contrôle d'accès
pub struct AccessControlManager {
    roles: HashMap<Role, RoleDefinition>,
    sessions: HashMap<SessionId, AuthenticatedSession>,
    session_timeout_seconds: u64,
}

impl AccessControlManager {
    pub fn new() -> Self {
        let mut roles = HashMap::new();

        // Mission Commander — accès total
        roles.insert(Role::MissionCommander, RoleDefinition {
            role: Role::MissionCommander,
            permissions: vec![
                Permission { action: "*".into(), resource: "*".into(), scope: PermissionScope::Global },
            ].into_iter().collect(),
            description: "Full access to all systems and data".into(),
            max_classification: crate::DataClassification::TopSecret,
        });

        // Flight Director — contrôle opérationnel
        roles.insert(Role::FlightDirector, RoleDefinition {
            role: Role::FlightDirector,
            permissions: vec![
                Permission { action: "command".into(), resource: "rover.*".into(), scope: PermissionScope::Global },
                Permission { action: "command".into(), resource: "habitat.*".into(), scope: PermissionScope::Global },
                Permission { action: "read".into(), resource: "telemetry.*".into(), scope: PermissionScope::Global },
                Permission { action: "execute".into(), resource: "scheduler.*".into(), scope: PermissionScope::Global },
            ].into_iter().collect(),
            description: "Operational command and telemetry access".into(),
            max_classification: crate::DataClassification::Secret,
        });

        // Scientist — données scientifiques
        roles.insert(Role::Scientist, RoleDefinition {
            role: Role::Scientist,
            permissions: vec![
                Permission { action: "read".into(), resource: "science_data.*".into(), scope: PermissionScope::Global },
                Permission { action: "analyze".into(), resource: "science_data.*".into(), scope: PermissionScope::Global },
                Permission { action: "read".into(), resource: "telemetry.*".into(), scope: PermissionScope::Global },
            ].into_iter().collect(),
            description: "Science data access and analysis".into(),
            max_classification: crate::DataClassification::Confidential,
        });

        // Operator — opérations de base
        roles.insert(Role::Operator, RoleDefinition {
            role: Role::Operator,
            permissions: vec![
                Permission { action: "read".into(), resource: "status.*".into(), scope: PermissionScope::Global },
                Permission { action: "monitor".into(), resource: "health.*".into(), scope: PermissionScope::Global },
            ].into_iter().collect(),
            description: "Monitoring and basic operations".into(),
            max_classification: crate::DataClassification::Internal,
        });

        // Autonomous System — actions système
        roles.insert(Role::AutonomousSystem, RoleDefinition {
            role: Role::AutonomousSystem,
            permissions: vec![
                Permission { action: "*".into(), resource: "internal.*".into(), scope: PermissionScope::System("spaceone".into()) },
                Permission { action: "read".into(), resource: "health.*".into(), scope: PermissionScope::Global },
                Permission { action: "actuate".into(), resource: "recovery.*".into(), scope: PermissionScope::Global },
            ].into_iter().collect(),
            description: "Autonomous system actions".into(),
            max_classification: crate::DataClassification::Secret,
        });

        Self {
            roles,
            sessions: HashMap::new(),
            session_timeout_seconds: 3600,
        }
    }

    /// Crée une session authentifiée pour un principal
    pub fn authenticate(&mut self, principal: String, role: Role, node_id: String) -> SecurityResult<SessionId> {
        let role_def = self.roles.get(&role)
            .ok_or_else(|| SecurityError::AccessDenied(format!("Unknown role: {:?}", role)))?;

        let session = AuthenticatedSession {
            session_id: SessionId(uuid::Uuid::new_v4().to_string()),
            principal,
            role,
            capabilities: role_def.permissions.clone(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(self.session_timeout_seconds as i64),
            node_id,
        };

        let id = session.session_id.clone();
        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Vérifie une permission pour une session
    pub fn check_permission(
        &self,
        session_id: &SessionId,
        action: &str,
        resource: &str,
    ) -> SecurityResult<bool> {
        let session = self.sessions.get(session_id)
            .ok_or_else(|| SecurityError::AccessDenied("Session not found".into()))?;

        if !session.is_valid() {
            return Err(SecurityError::AccessDenied("Session expired".into()));
        }

        // Wildcard permissions
        let has_wildcard = session.capabilities.iter().any(|p| {
            p.action == "*" && p.resource == "*"
        });

        if has_wildcard {
            return Ok(true);
        }

        Ok(session.has_permission(action, resource))
    }

    /// Termine une session
    pub fn terminate_session(&mut self, session_id: &SessionId) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    /// Nettoie les sessions expirées
    pub fn cleanup_expired_sessions(&mut self) -> usize {
        let initial = self.sessions.len();
        self.sessions.retain(|_, s| s.is_valid());
        initial - self.sessions.len()
    }

    /// Nombre de sessions actives
    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticate_and_check() {
        let mut acm = AccessControlManager::new();

        let session_id = acm.authenticate(
            "commander_shepard".into(),
            Role::MissionCommander,
            "rover-01".into(),
        ).unwrap();

        assert!(acm.check_permission(&session_id, "command", "rover.nav").unwrap());
        assert!(acm.check_permission(&session_id, "read", "science_data.images").unwrap());
    }

    #[test]
    fn test_scientist_limited_access() {
        let mut acm = AccessControlManager::new();

        let session_id = acm.authenticate(
            "dr_watson".into(),
            Role::Scientist,
            "science_lab".into(),
        ).unwrap();

        // Scientist peut lire les données science
        assert!(acm.check_permission(&session_id, "read", "science_data.samples").unwrap());
        // Scientist ne peut PAS commander un rover
        assert!(!acm.check_permission(&session_id, "command", "rover.nav").unwrap());
    }

    #[test]
    fn test_expired_session() {
        let mut acm = AccessControlManager::new();
        let session_id = acm.authenticate(
            "test".into(),
            Role::Operator,
            "test-node".into(),
        ).unwrap();

        // Forcer l'expiration
        let session = acm.sessions.get_mut(&session_id).unwrap();
        session.expires_at = Utc::now() - chrono::Duration::hours(1);

        assert!(acm.check_permission(&session_id, "read", "status").is_err());
    }

    #[test]
    fn test_session_cleanup() {
        let mut acm = AccessControlManager::new();

        for i in 0..5 {
            acm.authenticate(
                format!("user-{}", i),
                Role::Observer,
                "test".into(),
            ).unwrap();
        }

        // Expirer toutes les sessions
        for (_, session) in acm.sessions.iter_mut() {
            session.expires_at = Utc::now() - chrono::Duration::hours(1);
        }

        let cleaned = acm.cleanup_expired_sessions();
        assert_eq!(cleaned, 5);
        assert_eq!(acm.active_sessions(), 0);
    }
}
