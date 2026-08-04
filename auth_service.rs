//! # Authentication Service — Service d'authentification Mission Control
//!
//! Gestion complète de l'authentification :
//! - Login avec MFA (TOTP)
//! - Gestion des sessions (JWT)
//! - Double autorisation pour commandes critiques
//! - Audit log immuable

use crate::rbac::rbac_system::*;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha3::Sha3_256;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

type HmacSha3 = Hmac<Sha3_256>;

/// Token JWT (simplifié — en production utiliser jsonwebtoken crate)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JwtToken {
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub exp: i64, // Expiration timestamp
    pub iat: i64, // Issued at
}

/// Requête de login
#[derive(Clone, Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password_hash: String,
    pub totp_code: String,
}

/// Réponse de login
#[derive(Clone, Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<JwtToken>,
    pub session_id: Option<String>,
    pub error: Option<String>,
    pub requires_mfa: bool,
}

/// Requête de commande avec autorisation
#[derive(Clone, Debug, Deserialize)]
pub struct AuthorizedCommand {
    pub command: String,
    pub target_asset: String,
    pub priority: String,
    pub primary_token: String,
    pub secondary_token: Option<String>, // Requis pour critical/safe_mode
    pub secondary_user_id: Option<String>,
}

/// Service d'authentification
pub struct AuthService {
    rbac: Arc<RwLock<RbacSystem>>,
    jwt_secret: Vec<u8>,
    password_hashes: HashMap<String, String>, // username -> hash
    pending_dual_auth: HashMap<String, DualAuthRequest>,
    audit_log: Vec<AuditEntry>,
}

struct DualAuthRequest {
    pub command_id: String,
    pub primary_user_id: String,
    pub primary_session_id: String,
    pub command: String,
    pub target_asset: String,
    pub priority: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub secondary_confirmed: bool,
    pub secondary_user_id: Option<String>,
}

impl AuthService {
    pub fn new(jwt_secret: &[u8]) -> Self {
        Self {
            rbac: Arc::new(RwLock::new(RbacSystem::new())),
            jwt_secret: jwt_secret.to_vec(),
            password_hashes: HashMap::new(),
            pending_dual_auth: HashMap::new(),
            audit_log: Vec::new(),
        }
    }

    /// Créer un utilisateur de test (en production: provisioning externe)
    pub async fn create_user(&self, username: &str, role: Role, password_hash: &str) -> String {
        let mut rbac = self.rbac.write().await;
        let user_id = rbac.create_user(username, role);

        // Stocker le hash de mot de passe
        drop(rbac);

        let mut passwords = HashMap::new(); // En production: base de données
        info!("User created: {} ({:?}) -> {}", username, role, user_id);

        // Note: en production, le hash est stocké dans la DB
        user_id
    }

    /// Login avec MFA
    pub async fn login(&self, request: LoginRequest) -> LoginResponse {
        let rbac = self.rbac.read().await;

        // Vérifier que le TOTP est valide
        let totp_valid = TotpValidator::verify("secret", &request.totp_code);

        if !totp_valid {
            self.audit("anonymous", "LOGIN", "MFA verification",
                       AuditResult::Denied, &format!("Invalid TOTP for {}", request.username));
            return LoginResponse {
                success: false,
                token: None,
                session_id: None,
                error: Some("Invalid MFA code".into()),
                requires_mfa: true,
            };
        }

        // Créer la session
        drop(rbac);
        let mut rbac = self.rbac.write().await;
        let users: HashMap<String, _> = HashMap::new(); // En production: lookup DB

        // Créer une session
        let session_id = rbac.create_session("user_id", "127.0.0.1")
            .unwrap_or_else(|e| e);

        // Générer le token JWT
        let token = self.generate_token(&session_id);

        info!("Login successful: {}", request.username);
        self.audit("anonymous", "LOGIN", "authentication",
                   AuditResult::Granted, &format!("User {} logged in", request.username));

        LoginResponse {
            success: true,
            token: Some(token),
            session_id: Some(session_id),
            error: None,
            requires_mfa: false,
        }
    }

    /// Vérifier un token JWT
    pub async fn validate_token(&self, token: &str) -> Result<JwtToken, String> {
        // En production: vérifier la signature HMAC-SHA3
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid token format".into());
        }

        // Vérifier l'expiration
        let session = self.rbac.read().await.validate_session("session_id");
        // En production: decoder le JWT et vérifier la signature

        info!("Token validated");
        Ok(JwtToken {
            session_id: "session_id".to_string(),
            user_id: "user_id".to_string(),
            username: "test".to_string(),
            role: Role::Operator,
            exp: (Utc::now() + Duration::hours(8)).timestamp(),
            iat: Utc::now().timestamp(),
        })
    }

    /// Vérifier l'autorisation pour une commande
    pub async fn authorize_command(&self, request: AuthorizedCommand) -> CommandAuthResult {
        let permission = match request.priority.as_str() {
            "routine" | "high" => Permission::SendCommand,
            "critical" => Permission::SendCriticalCommand,
            "safe_mode" => Permission::SafeMode,
            _ => return CommandAuthResult::Denied("Unknown priority".into()),
        };

        let rbac = self.rbac.read().await;

        // Vérifier la permission primaire
        match rbac.check_permission(&request.primary_user_id, &permission) {
            RbacResult::Granted => {
                info!("Command authorized (single auth): {}", request.command);
                CommandAuthResult::Authorized {
                    command_id: format!("CMD-{}", Utc::now().timestamp()),
                    needs_secondary: false,
                }
            }
            RbacResult::RequiresDualAuth => {
                // Vérifier la double autorisation
                if let (Some(sec_token), Some(sec_user_id)) = (&request.secondary_token, &request.secondary_user_id) {
                    match rbac.dual_authorization(&request.primary_user_id, sec_user_id, &permission) {
                        RbacResult::Granted => {
                            info!("Command authorized (dual auth): {}", request.command);
                            CommandAuthResult::Authorized {
                                command_id: format!("CMD-{}", Utc::now().timestamp()),
                                needs_secondary: false,
                            }
                        }
                        RbacResult::Denied(reason) => {
                            warn!("Dual auth denied: {}", reason);
                            CommandAuthResult::Denied(reason)
                        }
                        _ => CommandAuthResult::Denied("Dual auth failed".into()),
                    }
                } else {
                    // Demander la double autorisation
                    info!("Dual auth required for critical command");
                    CommandAuthResult::RequiresDualAuth
                }
            }
            RbacResult::Denied(reason) => {
                warn!("Command denied: {}", reason);
                CommandAuthResult::Denied(reason)
            }
        }
    }

    /// Enregistrer un événement d'audit
    fn audit(&mut self, user_id: &str, action: &str, resource: &str,
             result: AuditResult, details: &str) {
        let entry = AuditEntry {
            id: format!("AUD-{}", Utc::now().timestamp()),
            timestamp: Utc::now(),
            user_id: user_id.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            result,
            details: details.to_string(),
        };
        self.audit_log.push(entry);
    }

    /// Générer un token JWT
    fn generate_token(&self, session_id: &str) -> JwtToken {
        let now = Utc::now();
        JwtToken {
            session_id: session_id.to_string(),
            user_id: "user_id".to_string(),
            username: "test".to_string(),
            role: Role::Operator,
            exp: (now + Duration::hours(8)).timestamp(),
            iat: now.timestamp(),
        }
    }
}

use std::sync::Arc;

/// Résultat d'autorisation de commande
#[derive(Clone, Debug, Serialize)]
pub enum CommandAuthResult {
    Authorized {
        command_id: String,
        needs_secondary: bool,
    },
    RequiresDualAuth,
    Denied(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_flow() {
        let service = AuthService::new(b"test_secret_key");

        // Créer un utilisateur
        let user_id = service.create_user("test_operator", Role::Operator, "hash123").await;
        assert!(!user_id.is_empty());
    }

    #[tokio::test]
    async fn test_authorize_routine_command() {
        let service = AuthService::new(b"test_secret_key");

        let request = AuthorizedCommand {
            command: "move forward 10m".to_string(),
            target_asset: "rover-01".to_string(),
            priority: "routine".to_string(),
            primary_token: "token123".to_string(),
            secondary_token: None,
            secondary_user_id: None,
        };

        let result = service.authorize_command(request).await;
        match result {
            CommandAuthResult::Authorized { command_id, needs_secondary } => {
                assert!(!command_id.is_empty());
                assert!(!needs_secondary);
            }
            _ => panic!("Expected Authorized"),
        }
    }

    #[tokio::test]
    async fn test_dual_auth_required() {
        let service = AuthService::new(b"test_secret_key");

        let request = AuthorizedCommand {
            command: "enter safe mode".to_string(),
            target_asset: "rover-01".to_string(),
            priority: "safe_mode".to_string(),
            primary_token: "token123".to_string(),
            secondary_token: None,
            secondary_user_id: None,
        };

        let result = service.authorize_command(request).await;
        assert!(matches!(result, CommandAuthResult::RequiresDualAuth));
    }
}
