//! # System Bridge — Gestion système réelle via systemctl/DBus
//!
//! Ce module fournit les opérations système réelles nécessaires à SpaceOne
//! pour interagir avec le système AsterQuanta :
//! - Démarrer/arrêter des services systemd
//! - Basculer en safe mode (aqm-safe.target)
//! - Lire les logs système (journald)
//! - Déclencher un reboot contrôlé
//!
//! Tous les appels sont des commandes système réelles.

use super::*;

/// Système bridge pour les opérations AsterQuanta
pub struct SystemBridge;

impl SystemBridge {
    // ─── Safe Mode ───

    /// Activer le safe mode d'AsterQuanta
    ///
    /// Bascule vers `aqm-safe.target` qui ne démarre que :
    /// - aqm-supervisor
    /// - sshd
    /// - aqm-shell (CLI)
    ///
    /// Arrête : aqm-ui, aqm-coordination, aqm-science
    pub fn enable_safe_mode() -> BridgeResult<()> {
        warn!("ACTIVATING SAFE MODE — isolating aqm-safe.target");

        let output = Command::new("systemctl")
            .args(["isolate", "aqm-safe.target"])
            .output()
            .map_err(|e| BridgeError::CommandFailed(format!(
                "systemctl isolate failed: {}", e
            )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BridgeError::CommandFailed(format!(
                "safe mode activation failed: {}", stderr
            )));
        }

        info!("Safe mode activated — critical services only");
        Ok(())
    }

    /// Désactiver le safe mode et revenir au mode nominal
    pub fn disable_safe_mode() -> BridgeResult<()> {
        info!("EXITING SAFE MODE — returning to multi-user.target");

        let output = Command::new("systemctl")
            .args(["isolate", "multi-user.target"])
            .output()
            .map_err(|e| BridgeError::CommandFailed(format!(
                "systemctl isolate failed: {}", e
            )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BridgeError::CommandFailed(format!(
                "safe mode deactivation failed: {}", stderr
            )));
        }

        info!("Safe mode deactivated — full system restored");
        Ok(())
    }

    /// Déclencher le mode recovery (kernel minimal + CLI)
    pub fn trigger_recovery() -> BridgeResult<()> {
        warn!("TRIGGERING RECOVERY MODE — isolating aqm-recovery.target");

        let output = Command::new("systemctl")
            .args(["isolate", "aqm-recovery.target"])
            .output()
            .map_err(|e| BridgeError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(BridgeError::CommandFailed(
                "Recovery mode activation failed".into()
            ));
        }

        info!("Recovery mode activated — minimal environment");
        Ok(())
    }

    // ─── Gestion des services ───

    /// Démarrer un service systemd
    pub fn start_service(service_name: &str) -> BridgeResult<()> {
        Self::run_systemctl(&["start", service_name])
    }

    /// Arrêter un service systemd
    pub fn stop_service(service_name: &str) -> BridgeResult<()> {
        Self::run_systemctl(&["stop", service_name])
    }

    /// Redémarrer un service systemd
    pub fn restart_service(service_name: &str) -> BridgeResult<()> {
        Self::run_systemctl(&["restart", service_name])
    }

    /// Vérifier si un service est actif
    pub fn is_service_active(service_name: &str) -> bool {
        let output = Command::new("systemctl")
            .args(["is-active", "--quiet", service_name])
            .output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Lister tous les services AQM actifs
    pub fn list_aqm_services() -> BridgeResult<Vec<String>> {
        let output = Command::new("systemctl")
            .args([
                "list-units",
                "aqm-*.service",
                "spaceone-*.service",
                "rauc.service",
                "--no-pager",
                "--plain",
            ])
            .output()
            .map_err(|e| BridgeError::CommandFailed(e.to_string()))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let services: Vec<String> = text.lines()
            .filter(|l| !l.is_empty() && l.contains(".service"))
            .map(|l| l.split_whitespace().next().unwrap_or("").to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(services)
    }

    // ─── Logs (journald) ───

    /// Lire les N dernières entrées de journal pour un service
    pub fn read_service_logs(service_name: &str, lines: usize) -> BridgeResult<String> {
        let output = Command::new("journalctl")
            .args([
                "-u",
                service_name,
                "-n",
                &lines.to_string(),
                "--no-pager",
                "--output=short-iso",
            ])
            .output()
            .map_err(|e| BridgeError::CommandFailed(e.to_string()))?;

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(text)
    }

    /// Lire les logs de tous les services SpaceOne
    pub fn read_spaceone_logs(lines: usize) -> BridgeResult<String> {
        let output = Command::new("journalctl")
            .args([
                "--unit=spaceone-*",
                "-n",
                &lines.to_string(),
                "--no-pager",
                "--output=short-iso",
            ])
            .output()
            .map_err(|e| BridgeError::CommandFailed(e.to_string()))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // ─── Reboot ───

    /// Redémarrage contrôlé (avec sauvegarde d'état)
    pub fn controlled_reboot(reason: &str) -> BridgeResult<()> {
        warn!("CONTROLLED REBOOT — reason: {}", reason);

        // Sauvegarder l'état courant dans les logs
        let _ = Command::new("logger")
            .args([
                "-t", "spaceone",
                "-p", "daemon.warning",
                &format!("Controlled reboot initiated: {}", reason),
            ])
            .output();

        // Redémarrer
        Command::new("reboot")
            .output()
            .map_err(|e| BridgeError::CommandFailed(format!(
                "reboot failed: {}", e
            )))?;

        // Note: si on arrive ici, le reboot a échoué
        Err(BridgeError::CommandFailed("reboot did not execute".into()))
    }

    // ─── Helpers ───

    /// Exécuter une commande systemctl
    fn run_systemctl(args: &[&str]) -> BridgeResult<()> {
        let output = Command::new("systemctl")
            .args(args)
            .output()
            .map_err(|e| BridgeError::CommandFailed(format!(
                "systemctl {} failed: {}", args.join(" "), e
            )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BridgeError::CommandFailed(format!(
                "systemctl {} failed: {}", args.join(" "), stderr
            )));
        }

        Ok(())
    }
}

/// Snapshot de l'état système complet (pour diagnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub timestamp: String,
    pub uptime_seconds: u64,
    pub active_slot: String,
    pub aqm_health_score: f32,
    pub aqm_system_state: String,
    pub spaceone_services_active: usize,
    pub pending_dtn_bundles: u32,
    pub safe_mode_active: bool,
    pub last_reboot_reason: String,
}

impl SystemSnapshot {
    /// Capturer un snapshot complet du système
    pub fn capture() -> BridgeResult<Self> {
        // Uptime
        let uptime_output = Command::new("cat")
            .arg("/proc/uptime")
            .output();

        let uptime_seconds = uptime_output.ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next()
                .and_then(|s| s.split('.').next())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0))
            .unwrap_or(0);

        // Slot actif
        let rauc_status = RaucClient::get_status();
        let active_slot = rauc_status.as_ref()
            .map(|s| s.boot_primary.clone())
            .unwrap_or_else(|_| "unknown".into());

        // Santé AQM
        let health = SupervisorClient::get_health_report();
        let (aqm_score, aqm_state) = match &health {
            Ok(h) => (h.score, h.system_state.clone()),
            Err(_) => (0.0, "unavailable".to_string()),
        };

        // Services SpaceOne
        let services = SystemBridge::list_aqm_services();
        let sp_active = services.as_ref()
            .map(|s| s.iter().filter(|svc| svc.starts_with("spaceone-")).count())
            .unwrap_or(0);

        // DTN pending
        let dtn_status = DtnClient::get_node_status();
        let pending = dtn_status.as_ref()
            .map(|s| s.pending_bundles)
            .unwrap_or(0);

        // Safe mode check
        let safe_active = SystemBridge::is_service_active("aqm-safe.target");

        // Reboot reason (last log entry)
        let last_reboot = SystemBridge::read_service_logs("spaceone-core", 1)
            .ok()
            .and_then(|l| l.lines().last().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        Ok(SystemSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            uptime_seconds,
            active_slot,
            aqm_health_score: aqm_score,
            aqm_system_state: aqm_state,
            spaceone_services_active: sp_active,
            pending_dtn_bundles: pending,
            safe_mode_active: safe_active,
            last_reboot_reason: last_reboot,
        })
    }
}
