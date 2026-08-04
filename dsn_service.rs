//! # DSN Service — Service d'intégration Deep Space Network
//!
//! Ce service fait le pont entre le Mission Control (command_relay.rs)
//! et le réseau DSN physique de la NASA. Il gère :
//! - La planification des time slots DSN (via API JPL)
//! - Le tracking de la position Mars
//! - La gestion des fenêtres de communication
//! - L'encapsulation CCSDS des bundles DTN

use crate::dsn::dsn_relay::*;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Configuration du service DSN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DsnServiceConfig {
    /// URL de l'API JPL Horizons (éphémérides Mars)
    pub jpl_horizons_url: String,
    /// API key pour le service DSN
    pub api_key: String,
    /// Fréquence du polling de statut (secondes)
    pub status_poll_interval: u64,
    /// Fenêtre de communication minimale (minutes)
    pub min_window_minutes: f64,
    /// Taux d'erreur maximum acceptable (BER)
    pub max_ber: f64,
}

impl Default for DsnServiceConfig {
    fn default() -> Self {
        Self {
            jpl_horizons_url: "https://ssd.jpl.nasa.gov/api/horizons.api".to_string(),
            api_key: String::new(), // Configuré via env var en production
            status_poll_interval: 60,
            min_window_minutes: 8.0,
            max_ber: 1e-5,
        }
    }
}

/// Statut du lien Mars-Terre
#[derive(Clone, Debug, Serialize)]
pub struct MarsLinkStatus {
    pub is_visible: bool,
    pub elevation_degrees: f64,
    pub distance_au: f64,
    pub light_time_minutes: f64,
    pub doppler_rate_hz_per_sec: f64,
    pub signal_delay_ms: f64,
    pub next_pass: Option<PassWindow>,
    pub solar_conjunction: bool,
}

/// Fenêtre de passage (pass window)
#[derive(Clone, Debug, Serialize)]
pub struct PassWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub duration_minutes: f64,
    pub max_elevation_degrees: f64,
    pub optimal_complex: String,
}

/// Solar Conjunction (Mars derrière le Soleil)
/// Période où toute communication est impossible (~2 semaines)
#[derive(Clone, Debug, Serialize)]
pub struct SolarConjunction {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub min_separation_degrees: f64,
}

/// Service DSN principal
pub struct DsnService {
    config: DsnServiceConfig,
    relay: Arc<RwLock<DsnRelay>>,
    link_status: Arc<RwLock<MarsLinkStatus>>,
    active_pass: Option<PassWindow>,
    conjunction: Option<SolarConjunction>,
}

impl DsnService {
    pub fn new(config: DsnServiceConfig, relay: DsnRelay) -> Self {
        Self {
            config,
            relay: Arc::new(RwLock::new(relay)),
            link_status: Arc::new(RwLock::new(MarsLinkStatus {
                is_visible: false,
                elevation_degrees: 0.0,
                distance_au: 1.5,
                light_time_minutes: 14.0,
                doppler_rate_hz_per_sec: 0.0,
                signal_delay_ms: 840_000.0,
                next_pass: None,
                solar_conjunction: false,
            })),
            active_pass: None,
            conjunction: None,
        }
    }

    /// Mettre à jour le statut du lien depuis JPL Horizons
    pub async fn update_link_status(&self) -> Result<MarsLinkStatus, String> {
        // En production: appel HTTP à l'API JPL Horizons
        // GET https://ssd.jpl.nasa.gov/api/horizons.api?format=json&COMMAND=MARS&...
        // Ici: calcul approximatif basé sur l'époque

        let now = Utc::now();
        let days_since_j2000 = (now - DateTime::parse_from_rfc3339("2000-01-01T12:00:00Z").unwrap()).num_days();

        // Orbite Mars: ~687 jours, excentricité ~0.093
        let mars_orbit_phase = (days_since_j2000 as f64 / 687.0).rem_euclid(1.0);
        let distance_au = 1.0 + 0.52 * (1.0 - 0.093 * (mars_orbit_phase * 2.0 * std::f64::consts::PI).cos());

        let light_time_min = distance_au * 8.317; // minutes lumière

        // Déterminer si un complexe DSN voit Mars
        // Simplifié: rotation Terre, 3 complexes espacés de 120°
        let hour_utc = now.hour() as f64;
        let complex_visible = match hour_utc as i32 {
            0..=7 => "DSS-43 (Canberra)",
            8..=15 => "DSS-54 (Madrid)",
            16..=23 => "DSS-14 (Goldstone)",
            _ => "DSS-14 (Goldstone)",
        };

        // Vérifier conjunction solaire (approximatif)
        let conjunction_period = (days_since_j2000 % 780) as f64;
        let in_conjunction = conjunction_period > 770.0;

        let status = MarsLinkStatus {
            is_visible: !in_conjunction,
            elevation_degrees: if in_conjunction { 0.0 } else { 45.0 + 20.0 * mars_orbit_phase.sin() },
            distance_au,
            light_time_minutes: light_time_min,
            doppler_rate_hz_per_sec: 2.5 * mars_orbit_phase.cos(),
            signal_delay_ms: light_time_min * 60_000.0,
            next_pass: if in_conjunction {
                None
            } else {
                Some(PassWindow {
                    start: now,
                    end: now + Duration::hours(8),
                    duration_minutes: 480.0,
                    max_elevation_degrees: 65.0,
                    optimal_complex: complex_visible.to_string(),
                })
            },
            solar_conjunction: in_conjunction,
        };

        let mut link = self.link_status.write().await;
        *link = status.clone();

        info!("DSN link: {:.1} AU, {:.1} min delay, {}", distance_au, light_time_min,
              if in_conjunction { "CONJUNCTION" } else { "VISIBLE" });

        Ok(status)
    }

    /// Démarrer le polling automatique
    pub async fn start_polling(&self) {
        let interval = self.config.status_poll_interval;
        loop {
            match self.update_link_status().await {
                Ok(_) => info!("DSN status updated"),
                Err(e) => error!("DSN status update failed: {}", e),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    }

    /// Envoyer un bundle via le DSN
    pub async fn send_bundle(&self, bundle: DsnBundle) -> Result<String, String> {
        // Vérifier la conjunction
        let status = self.link_status.read().await;
        if status.solar_conjunction {
            return Err("Solar conjunction — no DSN contact possible".to_string());
        }
        if !status.is_visible {
            return Err("Mars not visible — waiting for next pass".to_string());
        }

        // Vérifier le BER
        if status.signal_delay_ms > 1_500_000.0 {
            warn!("High signal delay: {}ms — using maximum FEC", status.signal_delay_ms);
        }

        drop(status);

        // Planifier la transmission
        let mut relay = self.relay.write().await;
        relay.select_active_complex();
        relay.schedule_transmission(bundle.clone())
    }

    /// Statut complet du service DSN
    pub async fn full_status(&self) -> DsnServiceStatus {
        let link = self.link_status.read().await;
        let relay = self.relay.read().await;

        DsnServiceStatus {
            link: link.clone(),
            dsn_stats: relay.stats(),
            active_complex: link.next_pass.as_ref().map(|p| p.optimal_complex.clone()),
            conjunction_active: link.solar_conjunction,
        }
    }
}

/// Statut complet du service DSN
#[derive(Clone, Debug, Serialize)]
pub struct DsnServiceStatus {
    pub link: MarsLinkStatus,
    pub dsn_stats: DsnStats,
    pub active_complex: Option<String>,
    pub conjunction_active: bool,
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_relay() -> DsnRelay {
        DsnRelay::new(DsnConfig {
            complexes: vec![DsnComplex {
                id: "DSS-14".into(),
                location: "Goldstone".into(),
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
            }],
            frequency_mhz: 32000.0,
            transmit_power_dbm: 43.0,
            receive_sensitivity_dbm: -160.0,
        })
    }

    #[tokio::test]
    async fn test_link_status_update() {
        let service = DsnService::new(DsnServiceConfig::default(), test_relay());
        let status = service.update_link_status().await.unwrap();
        assert!(status.distance_au > 0.5);
        assert!(status.light_time_minutes > 3.0);
    }

    #[tokio::test]
    async fn test_bundle_routing() {
        let service = DsnService::new(DsnServiceConfig::default(), test_relay());
        service.update_link_status().await.unwrap();

        let bundle = DsnBundle {
            id: "TEST-DSN-001".into(),
            payload: vec![0xAA, 0xBB, 0xCC],
            destination_eid: "dtn://mars/rover-01".into(),
            priority: 3,
            modulation: DsnModulation::PcmPskPm,
            scheduled_time: None,
            status: DsnBundleStatus::Scheduled,
        };

        let result = service.send_bundle(bundle).await;
        assert!(result.is_ok());
    }
}
