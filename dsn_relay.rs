//! # DSN Relay — Intégration Deep Space Network (NASA)
//!
//! Ce module gère la communication entre le Mission Control et le
//! Deep Space Network (DSN) de la NASA. Le DSN est l'infrastructure
//! radio qui transmet les signaux entre la Terre et Mars.
//!
//! Le DSN a 3 complexes :
//! - Goldstone (Californie, USA) — 34m + 70m
//! - Canberra (Australie) — 34m + 70m
//! - Madrid (Espagne) — 34m + 70m
//!
//! À tout moment, au moins un complexe a Mars en visibilité.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Configuration du DSN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnConfig {
    /// Complexes DSN disponibles
    pub complexes: Vec<DsnComplex>,
    /// Fréquence de communication (Ka-band)
    pub frequency_mhz: f64,
    /// Puissance d'émission (dBm)
    pub transmit_power_dbm: f64,
    /// Sensibilité de réception (dBm)
    pub receive_sensitivity_dbm: f64,
}

/// Complexe DSN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnComplex {
    pub id: String,
    pub location: String,
    pub dishes: Vec<DsnDish>,
    pub is_visible_mars: bool,
    pub next_pass_start: Option<String>,
    pub next_pass_end: Option<String>,
}

/// Antenne DSN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnDish {
    pub id: String,
    pub diameter_meters: f64,
    pub band: String,  // X-band, Ka-band, S-band
    pub is_available: bool,
    pub is_scheduled: bool,
}

/// Statut de la liaison DSN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnLinkStatus {
    pub active_complex: Option<String>,
    pub signal_strength_db: f64,
    pub snr_db: f64,
    pub bit_error_rate: f64,
    pub round_trip_light_time_minutes: f64,
    pub doppler_shift_hz: f64,
    pub next_window_start: Option<String>,
    pub next_window_duration_minutes: f64,
}

/// Bundle DSN en transit
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnBundle {
    pub id: String,
    pub payload: Vec<u8>,
    pub destination_eid: String,
    pub priority: u8,
    pub modulation: DsnModulation,
    pub scheduled_time: Option<String>,
    pub status: DsnBundleStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DsnModulation {
    PcmPskPm,     // PCM/PSK/PM (standard DSN)
    Bpsk,
    Qpsk,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DsnBundleStatus {
    Scheduled,
    Transmitting,
    InTransit,
    Delivered,
    Failed,
}

/// Gestionnaire DSN
pub struct DsnRelay {
    config: DsnConfig,
    active_complex: Option<String>,
    scheduled_bundles: Vec<DsnBundle>,
    transmit_queue: Vec<DsnBundle>,
    delivered_count: u64,
    failed_count: u64,
}

impl DsnRelay {
    pub fn new(config: DsnConfig) -> Self {
        Self {
            config,
            active_complex: None,
            scheduled_bundles: Vec::new(),
            transmit_queue: Vec::new(),
            delivered_count: 0,
            failed_count: 0,
        }
    }

    /// Déterminer le complexe DSN actif (celui qui voit Mars)
    pub fn select_active_complex(&mut self) -> Option<String> {
        let visible: Vec<&DsnComplex> = self.config.complexes.iter()
            .filter(|c| c.is_visible_mars)
            .collect();

        if visible.is_empty() {
            warn!("No DSN complex currently visible to Mars");
            self.active_complex = None;
            return None;
        }

        // Sélectionner le complexe avec le meilleur lien (SNR)
        let best = visible[0]; // Simplifié — en production: calculer le SNR
        let id = best.id.clone();

        info!("DSN active complex: {} ({})", id, best.location);
        self.active_complex = Some(id);
        Some(id)
    }

    /// Planifier l'envoi d'un bundle via le DSN
    pub fn schedule_transmission(&mut self, bundle: DsnBundle) -> Result<String, String> {
        // Vérifier qu'un complexe est disponible
        let complex = self.active_complex.as_ref()
            .ok_or_else(|| "No DSN complex available — waiting for Mars visibility".to_string())?;

        info!("Scheduling bundle {} via DSN complex {}", bundle.id, complex);

        let mut scheduled = bundle;
        scheduled.status = DsnBundleStatus::Scheduled;

        self.scheduled_bundles.push(scheduled.clone());
        Ok(bundle.id)
    }

    /// Transmettre les bundles planifiés (appelé quand le DSN est prêt)
    pub fn transmit_scheduled(&mut self) -> Vec<DsnBundle> {
        let mut transmitted = Vec::new();

        let ready: Vec<DsnBundle> = self.scheduled_bundles.drain(..)
            .filter(|b| b.status == DsnBundleStatus::Scheduled)
            .collect();

        for mut bundle in ready {
            // Simuler la transmission DSN
            // En production: appel à l'API DSN (JPL) pour programmer le time slot
            bundle.status = DsnBundleStatus::Transmitting;
            bundle.scheduled_time = Some(Utc::now().to_rfc3339());

            info!("DSN transmit: {} ({} bytes, {})", bundle.id, bundle.payload.len(),
                  match bundle.modulation {
                      DsnModulation::PcmPskPm => "PCM/PSK/PM",
                      DsnModulation::Bpsk => "BPSK",
                      DsnModulation::Qpsk => "QPSK",
                  });

            self.transmit_queue.push(bundle.clone());
            transmitted.push(bundle);
        }

        transmitted
    }

    /// Statut de la liaison DSN
    pub fn link_status(&self) -> DsnLinkStatus {
        // En production: calculer depuis les données de navigation
        let rltt = 14.0; // Round-trip light time (minutes) — variable 6-24 min

        DsnLinkStatus {
            active_complex: self.active_complex.clone(),
            signal_strength_db: -140.0, // dB (Mars distance)
            snr_db: 15.0,
            bit_error_rate: 1e-6,
            round_trip_light_time_minutes: rltt,
            doppler_shift_hz: 2500.0, // Hz (vitesse relative)
            next_window_start: Some(Utc::now().to_rfc3339()),
            next_window_duration_minutes: 8.0 * 60.0,
        }
    }

    /// Statistiques
    pub fn stats(&self) -> DsnStats {
        DsnStats {
            delivered: self.delivered_count,
            failed: self.failed_count,
            scheduled: self.scheduled_bundles.len(),
            in_queue: self.transmit_queue.len(),
            active_complex: self.active_complex.clone(),
        }
    }
}

/// Statistiques DSN
#[derive(Clone, Debug, Serialize)]
pub struct DsnStats {
    pub delivered: u64,
    pub failed: u64,
    pub scheduled: usize,
    pub in_queue: usize,
    pub active_complex: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> DsnConfig {
        DsnConfig {
            complexes: vec![
                DsnComplex {
                    id: "DSS-14".into(),
                    location: "Goldstone, California".into(),
                    dishes: vec![DsnDish {
                        id: "DSS-14-70m".into(),
                        diameter_meters: 70.0,
                        band: "Ka-band".into(),
                        is_available: true,
                        is_scheduled: false,
                    }],
                    is_visible_mars: true,
                    next_pass_start: None,
                    next_pass_end: None,
                },
            ],
            frequency_mhz: 32000.0, // Ka-band
            transmit_power_dbm: 43.0, // 20 kW
            receive_sensitivity_dbm: -160.0,
        }
    }

    #[test]
    fn test_select_complex() {
        let mut relay = DsnRelay::new(test_config());
        let complex = relay.select_active_complex();
        assert_eq!(complex, Some("DSS-14".to_string()));
    }

    #[test]
    fn test_schedule_and_transmit() {
        let mut relay = DsnRelay::new(test_config());
        relay.select_active_complex();

        let bundle = DsnBundle {
            id: "TEST-001".into(),
            payload: vec![1, 2, 3, 4],
            destination_eid: "dtn://mars/rover-01".into(),
            priority: 4,
            modulation: DsnModulation::PcmPskPm,
            scheduled_time: None,
            status: DsnBundleStatus::Scheduled,
        };

        relay.schedule_transmission(bundle).unwrap();
        let transmitted = relay.transmit_scheduled();
        assert_eq!(transmitted.len(), 1);
        assert_eq!(transmitted[0].status, DsnBundleStatus::Transmitting);
    }
}
