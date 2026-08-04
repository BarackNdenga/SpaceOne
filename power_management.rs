//! # Power Management — Low-Power Modes et Power Gating
//!
//! Gestion de l'énergie pour les plateformes martiennes :
//! - Power gating des circuits non-critiques
//! - Modes basse consommation (sleep, deep sleep)
//! - Protection contre SEL (Single Event Latch-up)

use serde::{Deserialize, Serialize};
use crate::HalResult;

/// Mode de consommation d'énergie
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerMode {
    /// Pleine puissance — toutes les fonctions actives
    FullPower,
    /// Réduit — circuits non-critiques désactivés
    Reduced,
    /// Sleep — seul le watchdog et la mémoire retenue
    Sleep,
    /// Deep Sleep — seul le power management actif
    DeepSleep,
    /// Hibernation — arrêt quasi-complet
    Hibernation,
}

/// État d'un composant sous power gating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerGatedComponent {
    pub name: String,
    pub is_active: bool,
    pub last_gated_at: Option<u64>, // timestamp ms
    pub gate_count: u32,
    pub power_mw: f64,
}

/// Configuration du power management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    pub max_power_budget_mw: f64,
    pub sel_detection_threshold_ma: f64,
    pub auto_gate_threshold_pct: u8,
    pub wake_sources: Vec<String>,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            max_power_budget_mw: 200.0,
            sel_detection_threshold_ma: 50.0,
            auto_gate_threshold_pct: 80,
            wake_sources: vec![
                "watchdog".into(),
                "timer".into(),
                "external_interrupt".into(),
            ],
        }
    }
}

/// Gestionnaire de puissance
pub struct PowerManager {
    config: PowerConfig,
    current_mode: PowerMode,
    components: Vec<PowerGatedComponent>,
    total_power_mw: f64,
    sel_events: u32,
}

impl PowerManager {
    pub fn new(config: PowerConfig) -> Self {
        Self {
            config,
            current_mode: PowerMode::FullPower,
            components: Vec::new(),
            total_power_mw: 0.0,
            sel_events: 0,
        }
    }

    /// Enregistre un composant géré par power gating
    pub fn register_component(&mut self, name: String, power_mw: f64) {
        self.components.push(PowerGatedComponent {
            name,
            is_active: true,
            last_gated_at: None,
            gate_count: 0,
            power_mw,
        });
        self.total_power_mw += power_mw;
    }

    /// Active le power gating d'un composant
    pub fn gate_component(&mut self, name: &str) -> HalResult<()> {
        for comp in &mut self.components {
            if comp.name == name && comp.is_active {
                comp.is_active = false;
                comp.last_gated_at = Some(0); // Simulé
                comp.gate_count += 1;
                self.total_power_mw -= comp.power_mw;
                return Ok(());
            }
        }
        Ok(()) // Composant déjà gated ou non trouvé
    }

    /// Réactive un composant gated
    pub fn ungate_component(&mut self, name: &str) -> HalResult<()> {
        for comp in &mut self.components {
            if comp.name == name && !comp.is_active {
                comp.is_active = true;
                self.total_power_mw += comp.power_mw;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Change le mode de puissance
    pub fn set_mode(&mut self, mode: PowerMode) {
        self.current_mode = mode;

        match mode {
            PowerMode::FullPower => {
                for comp in &mut self.components {
                    comp.is_active = true;
                }
                self.total_power_mw = self.components.iter().map(|c| c.power_mw).sum();
            }
            PowerMode::Reduced => {
                // Gater les composants non-critiques
                for comp in &mut self.components {
                    if !comp.name.contains("critical") && !comp.name.contains("life") {
                        let _ = self.gate_component(&comp.name);
                    }
                }
            }
            PowerMode::Sleep | PowerMode::DeepSleep | PowerMode::Hibernation => {
                // Gater tout sauf le watchdog et la mémoire
                for comp in &mut self.components {
                    if !comp.name.contains("watchdog") && !comp.name.contains("memory") {
                        let _ = self.gate_component(&comp.name);
                    }
                }
            }
        }
    }

    /// Détecte un événement SEL
    pub fn detect_sel(&mut self, current_draw_ma: f64) -> bool {
        if current_draw_ma > self.config.sel_detection_threshold_ma {
            self.sel_events += 1;
            // Power gate immédiat du composant suspect
            self.set_mode(PowerMode::Reduced);
            return true;
        }
        false
    }

    pub fn get_mode(&self) -> &PowerMode {
        &self.current_mode
    }

    pub fn get_total_power(&self) -> f64 {
        self.total_power_mw
    }

    pub fn get_sel_count(&self) -> u32 {
        self.sel_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_gating() {
        let mut pm = PowerManager::new(PowerConfig::default());
        pm.register_component("camera".into(), 50.0);
        pm.register_component("processor_critical".into(), 100.0);

        pm.gate_component("camera").unwrap();
        assert!((pm.total_power_mw - 100.0).abs() < 0.01);

        pm.ungate_component("camera").unwrap();
        assert!((pm.total_power_mw - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_sel_detection() {
        let mut pm = PowerManager::new(PowerConfig::default());
        let sel = pm.detect_sel(100.0); // > threshold 50mA
        assert!(sel);
        assert_eq!(pm.get_sel_count(), 1);
    }

    #[test]
    fn test_sleep_mode() {
        let mut pm = PowerManager::new(PowerConfig::default());
        pm.register_component("processor".into(), 100.0);
        pm.register_component("camera".into(), 50.0);
        pm.register_component("watchdog_timer".into(), 5.0);

        pm.set_mode(PowerMode::Sleep);
        assert!((pm.total_power_mw - 5.0).abs() < 0.01); // Seul watchdog
    }
}
