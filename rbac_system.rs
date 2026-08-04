//! # RBAC + MFA — Contrôle d'accès du Mission Control
//!
//! Système d'authentification et d'autorisation pour le Mission Control.
//! Implémente :
//! - RBAC (Role-Based Access Control) avec 5 rôles
//! - MFA (Multi-Factor Authentication) via TOTP
//! - Double autorisation pour les commandes critiques
//! - Audit log complet

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};
use std::collections::HashMap;
use tracing::{info, warn};

// ─── Rôles ───

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Role {
    /// Mission Commander — Autorisation totale
    MissionCommander,
    /// Flight Director — Commandes opérationnelles
    FlightDirector,
    /// Scientist — Accès aux données scientifiques uniquement
    Scientist,
    /// Engineer — Accès aux systèmes et maintenance
    Engineer,
    /// Operator — Accès limité (lecture + commandes routine)
    Operator,
}

impl Role {
    /// Permissions par rôle
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            Role::MissionCommander => vec![
                Permission::SendCommand,
                Permission::SendCriticalCommand,
                Permission::SafeMode,
                Permission::Reboot,
                Permission::FirmwareUpdate,
                Permission::ViewScienceData,
                Permission::ModifySchedule,
                Permission::ViewAllTelemetry,
                Permission::ManageUsers,
            ],
            Role::FlightDirector => vec![
                Permission::SendCommand,
                Permission::SendCriticalCommand,
                Permission::SafeMode,
                Permission::ViewScienceData,
                Permission::ModifySchedule,
                Permission::ViewAllTelemetry,
            ],
            Role::Scientist => vec![
                Permission::ViewScienceData,
                Permission::RequestScienceObservation,
            ],
            Role::Engineer => vec![
                Permission::SendCommand,
                Permission::ViewAllTelemetry,
                Permission::FirmwareUpdate,
                Permission::ViewSystemLogs,
            ],
            Role::Operator => vec![
                Permission::SendCommand,
                Permission::ViewAllTelemetry,
                Permission::ViewSystemLogs,
            ],
        }
    }

    /// Niveau de priorité (plus bas = plus de pouvoir)
    pub fn priority_level(&self) -> u8 {
        match self {
            Role::MissionCommander => 0,
            Role::FlightDirector => 1,
            Role::Engineer => 2,
            Role::Scientist => 3,
            Role::Operator => 4,
        }
    }
}

// ─── Permissions ───

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    SendCommand,
    SendCriticalCommand,
    SafeMode,
    Reboot,
    FirmwareUpdate,
    ViewScienceData,
    RequestScienceObservation,
    ModifySchedule,
    ViewAllTelemetry,
    ViewSystemLogs,
    ManageUsers,
}

// ─── Utilisateur ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub role: Role,
    pub mfa_enabled: bool,
    pub mfa_secret: Option<String>,
    pub active_session: bool,
    pub last_login: Option<String>,
    pub failed_attempts: u32,
    pub locked: bool,
}

// ─── Session ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub mfa_verified: bool,
    pub ip_address: String,
}

// ─── Audit Entry ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub result: AuditResult,
    pub details: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuditResult {
    Granted,
    Denied,
    RequiresDualAuth,
}

// ─── RBAC System ───

pub struct RbacSystem {
    users: HashMap<String, User>,
    sessions: HashMap<String, Session>,
    audit_log: Vec<AuditEntry>,
    session_timeout_minutes: u64,
    max_failed_attempts: u32,
}

impl RbacSystem {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            sessions: HashMap::new(),
            audit_log: Vec::new(),
            session_timeout_minutes: 480, // 8 heures
            max_failed_attempts: 5,
        }
    }

    /// Créer un utilisateur
    pub fn create_user(&mut self, username: &str, role: Role) -> String {
        let id = format!("USR-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap());

        let user = User {
            id: id.clone(),
            username: username.to_string(),
            role,
            mfa_enabled: true, // MFA obligatoire en production
            mfa_secret: None,
            active_session: false,
            last_login: None,
            failed_attempts: 0,
            locked: false,
        };

        info!("User created: {} ({:?})", username, role);
        self.users.insert(id.clone(), user);
        id
    }

    /// Vérifier si un utilisateur a une permission
    pub fn check_permission(&self, user_id: &str, permission: &Permission) -> RbacResult {
        let user = match self.users.get(user_id) {
            Some(u) => u,
            None => return RbacResult::Denied("User not found".into()),
        };

        if user.locked {
            return RbacResult::Denied("Account locked".into());
        }

        let has_permission = user.role.permissions().contains(permission);

        if !has_permission {
            warn!("Permission denied: {} lacks {:?}", user.username, permission);
            return RbacResult::Denied(format!("Role {:?} lacks permission {:?}", user.role, permission));
        }

        // Vérifier si la permission nécessite une double autorisation
        let needs_dual_auth = match permission {
            Permission::SendCriticalCommand | Permission::SafeMode | Permission::Reboot => true,
            _ => false,
        };

        if needs_dual_auth {
            return RbacResult::RequiresDualAuth;
        }

        RbacResult::Granted
    }

    /// Double autorisation (deux utilisateurs avec rôle suffisant)
    pub fn dual_authorization(&self, primary_user_id: &str, secondary_user_id: &str,
                               permission: &Permission) -> RbacResult {
        let primary = match self.users.get(primary_user_id) {
            Some(u) => u,
            None => return RbacResult::Denied("Primary user not found".into()),
        };
        let secondary = match self.users.get(secondary_user_id) {
            Some(u) => u,
            None => return RbacResult::Denied("Secondary user not found".into()),
        };

        // Les deux doivent avoir la permission
        let primary_ok = primary.role.permissions().contains(permission);
        let secondary_ok = secondary.role.permissions().contains(permission);

        if !primary_ok || !secondary_ok {
            return RbacResult::Denied("Insufficient permissions for dual auth".into());
        }

        // Les deux doivent être des utilisateurs différents
        if primary_user_id == secondary_user_id {
            return RbacResult::Denied("Self-authorization not allowed".into());
        }

        // Au moins un doit être MissionCommander ou FlightDirector
        let has_commander = primary.role == Role::MissionCommander
            || primary.role == Role::FlightDirector
            || secondary.role == Role::MissionCommander
            || secondary.role == Role::FlightDirector;

        if !has_commander {
            return RbacResult::Denied("At least one Flight Director required for dual auth".into());
        }

        RbacResult::Granted
    }

    /// Enregistrer un événement d'audit
    pub fn audit(&mut self, user_id: &str, action: &str, resource: &str,
                 result: AuditResult, details: &str) {
        let entry = AuditEntry {
            id: format!("AUD-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap()),
            timestamp: Utc::now(),
            user_id: user_id.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            result,
            details: details.to_string(),
        };
        self.audit_log.push(entry);
    }

    /// Récupérer le log d'audit
    pub fn get_audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Créer une session (après login + MFA)
    pub fn create_session(&mut self, user_id: &str, ip: &str) -> Result<String, String> {
        let user = self.users.get(user_id)
            .ok_or("User not found")?;

        if user.locked {
            return Err("Account locked".into());
        }

        if !user.mfa_enabled {
            return Err("MFA required but not enabled".into());
        }

        let session_id = format!("SES-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap());

        let session = Session {
            id: session_id.clone(),
            user_id: user_id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(self.session_timeout_minutes as i64),
            mfa_verified: true,
            ip_address: ip.to_string(),
        };

        self.sessions.insert(session_id.clone(), session);

        // Mettre à jour l'utilisateur
        if let Some(u) = self.users.get_mut(user_id) {
            u.active_session = true;
            u.last_login = Some(Utc::now().to_rfc3339());
            u.failed_attempts = 0;
        }

        Ok(session_id)
    }

    /// Vérifier si une session est valide
    pub fn validate_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id).filter(|s| {
            Utc::now() < s.expires_at
        })
    }
}

pub enum RbacResult {
    Granted,
    Denied(String),
    RequiresDualAuth,
}

// ─── TOTP (MFA) ───

pub struct TotpValidator;

impl TotpValidator {
    /// Générer un secret TOTP
    pub fn generate_secret() -> String {
        // En production: utiliser la crate totp-rs
        let secret = rand::random::<u64>();
        hex::encode(secret.to_be_bytes())
    }

    /// Vérifier un code TOTP
    pub fn verify(secret: &str, code: &str) -> bool {
        // En production: totp_rs::TOTP::new(...).check(code)
        // Ici: placeholder pour la structure
        code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
    }
}
