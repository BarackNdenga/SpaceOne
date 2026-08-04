//! # Supervisor IPC — Communication réelle avec aqm-supervisor
//!
//! Ce module communique avec le superviseur AsterQuanta via le socket UNIX
//! `/run/aqm/aqm-supervisor.sock`. C'est la même interface que `aqmctl status`.
//! Aucune simulation — le socket est réel et le superviseur est un vrai
//! daemon systemd qui interroge les services via `systemctl show`.

use super::*;

/// Client IPC vers aqm-supervisor
pub struct SupervisorClient;

impl SupervisorClient {
    /// Vérifier si le superviseur est joignable
    pub fn is_available() -> bool {
        std::path::Path::new(AQM_SUPERVISOR_SOCKET).exists()
    }

    /// Obtenir le rapport de santé complet d'AsterQuanta
    ///
    /// Appelle le socket UNIX du superviseur et désérialise le JSON retourné.
    /// Le superviseur retourne un score de santé + état des services systemd.
    pub fn get_health_report() -> BridgeResult<AqmHealthReport> {
        // Ouvrir le socket UNIX
        let mut stream = UnixStream::connect(AQM_SUPERVISOR_SOCKET)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "aqm-supervisor inaccessible ({})", e
            )))?;

        // Envoyer la requête "status"
        stream.write_all(b"status")
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "write failed: {}", e
            )))?;

        // Lire la réponse JSON
        let mut response = String::new();
        stream.read_to_string(&mut response)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "read failed: {}", e
            )))?;

        // Désérialiser
        let report: AqmHealthReport = serde_json::from_str(&response)
            .map_err(|e| BridgeError::Deserialization(format!(
                "Invalid supervisor response: {} (raw: {})", e, response
            )))?;

        info!(
            "AsterQuanta health: score={:.2}, state={}",
            report.score, report.system_state
        );

        Ok(report)
    }

    /// Interroger un service spécifique via systemctl show
    pub fn query_service(name: &str) -> BridgeResult<AqmServiceState> {
        let output = Command::new("systemctl")
            .args([
                "show",
                &format!("{}.service", name),
                "--property=ActiveState,SubState,NRestarts",
                "--value",
            ])
            .output()
            .map_err(|e| BridgeError::CommandFailed(format!(
                "systemctl failed: {}", e
            )))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines();

        Ok(AqmServiceState {
            service: name.to_string(),
            active_state: lines.next().unwrap_or("unknown").to_string(),
            sub_state: lines.next().unwrap_or("unknown").to_string(),
            restarts: lines.next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0),
        })
    }
}

/// Décision de safe mode basée sur le rapport de santé
pub fn evaluate_safe_mode_trigger(report: &AqmHealthReport) -> SafeModeDecision {
    match report.system_state.as_str() {
        "critical" => {
            warn!("CRITICAL: AsterQuanta system degraded — evaluating safe mode");
            let critical_services_down: Vec<&str> = report.services.iter()
                .filter(|s| s.active_state != "active")
                .map(|s| s.service.as_str())
                .collect();

            if critical_services_down.contains(&"aqm-dtnd.service") {
                // Communication DTN perdue — safe mode immédiat
                SafeModeDecision::Immediate {
                    reason: "aqm-dtnd unavailable — communication lost".into(),
                }
            } else if critical_services_down.contains(&"aqm-recovery.service") {
                // Recovery system perdue — safe mode immédiat
                SafeModeDecision::Immediate {
                    reason: "aqm-recovery unavailable — no rollback capability".into(),
                }
            } else {
                SafeModeDecision::Immediate {
                    reason: format!(
                        "{} critical services down",
                        critical_services_down.len()
                    ),
                }
            }
        }
        "degraded" => {
            if report.score < 0.6 {
                SafeModeDecision::Warning {
                    reason: format!("System score {:.2} below threshold 0.6", report.score),
                    threshold: 0.6,
                }
            } else {
                SafeModeDecision::Nominal
            }
        }
        "nominal" => SafeModeDecision::Nominal,
        other => SafeModeDecision::Unknown(other.to_string()),
    }
}

/// Décision de safe mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafeModeDecision {
    Nominal,
    Immediate { reason: String },
    Warning { reason: String, threshold: f32 },
    Unknown(String),
}
