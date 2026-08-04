//! # Watchdog Hardware — Recovery Hardware
//!
//! Watchdog matériel indépendant qui surveille le logiciel embarqué
//! et déclenche un reset hardware en cas de blocage.

use serde::{Deserialize, Serialize};
use crate::HalResult;

/// État du watchdog
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchdogState {
    Armed,
    Running,
    Expired,
    Reset,
    Disabled,
}

/// Configuration du watchdog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    pub timeout_ms: u32,
    pub max_missed_feeds: u32,
    pub auto_reset: bool,
    pub feed_interval_ms: u32,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            max_missed_feeds: 3,
            auto_reset: true,
            feed_interval_ms: 2000,
        }
    }
}

/// Watchdog hardware simulé
pub struct HardwareWatchdog {
    config: WatchdogConfig,
    state: WatchdogState,
    last_feed_time_ms: u64,
    missed_feeds: u32,
    total_resets: u32,
    uptime_ms: u64,
}

impl HardwareWatchdog {
    pub fn new(config: WatchdogConfig) -> Self {
        Self {
            config,
            state: WatchdogState::Armed,
            last_feed_time_ms: 0,
            missed_feeds: 0,
            total_resets: 0,
            uptime_ms: 0,
        }
    }

    /// Initialise le watchdog (après le boot)
    pub fn start(&mut self) {
        self.state = WatchdogState::Running;
        self.last_feed_time_ms = self.uptime_ms;
    }

    /// Nourrit le watchdog (heartbeat)
    pub fn feed(&mut self) {
        if self.state == WatchdogState::Running {
            self.last_feed_time_ms = self.uptime_ms;
            self.missed_feeds = 0;
        }
    }

    /// Tick du watchdog (appelé périodiquement)
    pub fn tick(&mut self, elapsed_ms: u64) -> bool {
        self.uptime_ms += elapsed_ms;

        if self.state != WatchdogState::Running {
            return false;
        }

        let time_since_feed = self.uptime_ms - self.last_feed_time_ms;
        if time_since_feed > self.config.timeout_ms as u64 {
            self.missed_feeds += 1;

            if self.missed_feeds >= self.config.max_missed_feeds {
                self.state = WatchdogState::Expired;
                if self.config.auto_reset {
                    self.state = WatchdogState::Reset;
                    self.total_resets += 1;
                    // Simuler le reset
                    self.last_feed_time_ms = self.uptime_ms;
                    self.missed_feeds = 0;
                    self.state = WatchdogState::Running;
                }
                return true; // Reset déclenché
            }
        }

        false
    }

    /// Désactive le watchdog
    pub fn disable(&mut self) {
        self.state = WatchdogState::Disabled;
    }

    pub fn get_state(&self) -> &WatchdogState {
        &self.state
    }

    pub fn get_stats(&self) -> WatchdogStats {
        WatchdogStats {
            state: self.state.clone(),
            uptime_ms: self.uptime_ms,
            missed_feeds: self.missed_feeds,
            total_resets: self.total_resets,
            time_since_last_feed_ms: self.uptime_ms - self.last_feed_time_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WatchdogStats {
    pub state: WatchdogState,
    pub uptime_ms: u64,
    pub missed_feeds: u32,
    pub total_resets: u32,
    pub time_since_last_feed_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_normal_operation() {
        let mut wd = HardwareWatchdog::new(WatchdogConfig::default());
        wd.start();
        assert_eq!(*wd.get_state(), WatchdogState::Running);

        // Nourrir régulièrement
        for i in 0..5 {
            wd.feed();
            wd.tick(1000);
        }

        let stats = wd.get_stats();
        assert_eq!(stats.total_resets, 0);
        assert_eq!(stats.missed_feeds, 0);
    }

    #[test]
    fn test_watchdog_trigger_reset() {
        let config = WatchdogConfig {
            timeout_ms: 2000,
            max_missed_feeds: 2,
            auto_reset: true,
            feed_interval_ms: 1000,
        };

        let mut wd = HardwareWatchdog::new(config);
        wd.start();
        wd.feed();

        // Ne pas nourrir — le watchdog devrait déclencher
        for _ in 0..10 {
            let triggered = wd.tick(1000);
            if triggered {
                break;
            }
        }

        let stats = wd.get_stats();
        assert_eq!(stats.total_resets, 1);
    }
}
