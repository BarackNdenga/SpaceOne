//! # Command Relay — Envoi asynchrone de commandes via DTN
//!
//! Ce module gère l'envoi des commandes Mission Control vers les assets
//! martiens via des bundles DTN. Les commandes sont asynchrones :
//! - Latence Mars-Terre : 3 à 20 minutes (one-way)
//! - Pas de confirmation instantanée
//! - File de commandes avec priorité
//! - Retry automatique en cas d'échec de transmission

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{info, warn, error};
use uuid::Uuid;

// ─── Modèles ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DtnCommand {
    pub id: String,
    pub command_type: CommandType,
    pub target: String,
    pub payload: Vec<u8>,
    pub priority: CommandPriority,
    pub max_lifetime_hours: u32,
    pub created_at: DateTime<Utc>,
    pub status: RelayStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommandType {
    /// Commande directe au rover/habitat
    Direct { action: String, params: String },
    /// Commande de mise à jour firmware
    FirmwareUpdate { bundle_id: String },
    /// Commande de safe mode
    SafeMode { reason: String },
    /// Commande de recovery
    Recovery { snapshot_name: Option<String> },
    /// Commande de redémarrage
    Reboot { reason: String },
    /// Commande scientifique
    Science { instrument: String, duration_seconds: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CommandPriority {
    Routine,
    High,
    Critical,
    SafeMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RelayStatus {
    Queued,
    Transmitting,
    InTransit,
    Delivered,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

// ─── Relay ───

pub struct CommandRelay {
    queue: VecDeque<DtnCommand>,
    in_transit: Vec<DtnCommand>,
    completed: Vec<DtnCommand>,
    max_concurrent: usize,
    dtn_endpoint: String,
}

impl CommandRelay {
    pub fn new(dtn_endpoint: &str) -> Self {
        Self {
            queue: VecDeque::new(),
            in_transit: Vec::new(),
            completed: Vec::new(),
            max_concurrent: 3, // Limite DSN
            dtn_endpoint: dtn_endpoint.to_string(),
        }
    }

    /// Enfiler une commande
    pub fn enqueue(&mut self, command: DtnCommand) -> String {
        let id = command.id.clone();
        info!("Command enqueued: {} (priority: {:?})", id, command.priority);
        self.queue.push_back(command);
        id
    }

    /// Traiter la file de commandes (appelé périodiquement)
    pub fn process_queue(&mut self) -> Vec<RelayEvent> {
        let mut events = Vec::new();

        // Trier par priorité (SafeMode > Critical > High > Routine)
        let mut queue: Vec<DtnCommand> = self.queue.drain(..).collect();
        queue.sort_by(|a, b| a.priority_sort_key().cmp(&b.priority_sort_key()));

        for cmd in queue {
            if self.in_transit.len() >= self.max_concurrent {
                // File pleine, remettre en queue
                self.queue.push_back(cmd);
                break;
            }

            // Simuler l'envoi via DTN (en production: appel réseau réel)
            match self.transmit_command(&cmd) {
                Ok(()) => {
                    let mut cmd = cmd;
                    cmd.status = RelayStatus::InTransit;
                    events.push(RelayEvent {
                        event_type: "command_sent".into(),
                        command_id: cmd.id.clone(),
                        message: format!("Command sent to {}", cmd.target),
                    });
                    self.in_transit.push(cmd);
                }
                Err(e) => {
                    let mut cmd = cmd;
                    cmd.retry_count += 1;
                    cmd.error = Some(e.clone());

                    if cmd.retry_count >= cmd.max_retries {
                        cmd.status = RelayStatus::Failed;
                        events.push(RelayEvent {
                            event_type: "command_failed".into(),
                            command_id: cmd.id.clone(),
                            message: format!("Failed after {} retries: {}", cmd.retry_count, e),
                        });
                        self.completed.push(cmd);
                    } else {
                        self.queue.push_back(cmd);
                        events.push(RelayEvent {
                            event_type: "retry_scheduled".into(),
                            command_id: cmd.id.clone(),
                            message: format!("Retry {}/{}", cmd.retry_count + 1, cmd.max_retries),
                        });
                    }
                }
            }
        }

        events
    }

    /// Transmettre un bundle via DTN (en production: socket UDP vers DSN)
    fn transmit_command(&self, cmd: &DtnCommand) -> Result<(), String> {
        info!("Transmitting command {} to {} via DTN...", cmd.id, self.dtn_endpoint);

        // En production : envoi réel via le Deep Space Network
        // Format du bundle DTN : header + payload + signature
        let bundle_size = cmd.payload.len() + 128; // header overhead

        // Vérifier les contraintes
        if bundle_size > 65536 {
            return Err("Bundle exceeds max size (64KB)".into());
        }

        // Simuler la latence de transmission (en production: temps réel)
        // Mars-Terre : 3 à 20 min selon la position orbitale

        Ok(())
    }

    /// Vérifier les commandes en transit pour mise à jour de statut
    pub fn check_transit_status(&mut self) -> Vec<RelayEvent> {
        let mut events = Vec::new();
        let mut delivered = Vec::new();

        for (i, cmd) in self.in_transit.iter().enumerate() {
            // En production : vérifier les accusés de réception DTN
            // Ici : simuler une livraison après timeout
            let age = Utc::now() - cmd.created_at;
            if age > Duration::from_secs(1200) { // 20 min max transit
                events.push(RelayEvent {
                    event_type: "delivery_timeout".into(),
                    command_id: cmd.id.clone(),
                    message: "Delivery timeout — assuming loss".into(),
                });
                delivered.push(i);
            }
        }

        // Retirer les commandes livrées/expirées (index inversé pour stabilité)
        for i in delivered.into_iter().rev() {
            let mut cmd = self.in_transit.remove(i);
            cmd.status = RelayStatus::Failed;
            cmd.error = Some("Delivery timeout".into());
            self.completed.push(cmd);
        }

        events
    }

    /// Annuler une commande en attente
    pub fn cancel_command(&mut self, id: &str) -> bool {
        if let Some(pos) = self.queue.iter().position(|c| c.id == id) {
            self.queue.remove(pos);
            info!("Command cancelled: {}", id);
            true
        } else {
            false
        }
    }

    /// Statistiques du relay
    pub fn stats(&self) -> RelayStats {
        RelayStats {
            queued: self.queue.len(),
            in_transit: self.in_transit.len(),
            completed_total: self.completed.len(),
            failed_total: self.completed.iter().filter(|c| c.status == RelayStatus::Failed).count(),
            max_concurrent: self.max_concurrent,
        }
    }
}

/// Statistiques du relay
#[derive(Clone, Debug, Serialize)]
pub struct RelayStats {
    pub queued: usize,
    pub in_transit: usize,
    pub completed_total: usize,
    pub failed_total: usize,
    pub max_concurrent: usize,
}

/// Événement du relay
#[derive(Clone, Debug, Serialize)]
pub struct RelayEvent {
    pub event_type: String,
    pub command_id: String,
    pub message: String,
}

impl DtnCommand {
    pub fn new(target: &str, command_type: CommandType, priority: CommandPriority) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            command_type,
            target: target.to_string(),
            payload: Vec::new(),
            priority,
            max_lifetime_hours: 24,
            created_at: Utc::now(),
            status: RelayStatus::Queued,
            retry_count: 0,
            max_retries: 3,
            error: None,
        }
    }

    fn priority_sort_key(&self) -> u8 {
        match self.priority {
            CommandPriority::SafeMode => 0,
            CommandPriority::Critical => 1,
            CommandPriority::High => 2,
            CommandPriority::Routine => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_and_process() {
        let mut relay = CommandRelay::new("dtn://mars/");

        let cmd = DtnCommand::new(
            "rover-01",
            CommandType::Direct { action: "move".into(), params: "forward 10m".into() },
            CommandPriority::Routine,
        );

        relay.enqueue(cmd);
        assert_eq!(relay.stats().queued, 1);

        let events = relay.process_queue();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "command_sent");
    }

    #[test]
    fn test_priority_ordering() {
        let mut relay = CommandRelay::new("dtn://mars/");

        relay.enqueue(DtnCommand::new("r1", CommandType::Direct { action: "a".into(), params: "".into() }, CommandPriority::Routine));
        relay.enqueue(DtnCommand::new("r2", CommandType::Direct { action: "b".into(), params: "".into() }, CommandPriority::SafeMode));
        relay.enqueue(DtnCommand::new("r3", CommandType::Direct { action: "c".into(), params: "".into() }, CommandPriority::Critical));

        let events = relay.process_queue();
        // SafeMode devrait être envoyé en premier
        assert!(events.iter().any(|e| e.command_id.contains("r2")));
    }
}
