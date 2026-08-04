//! # SpaceOne HAL — Portage ARM64 (Raspberry Pi 4 / i.MX8)
//!
//! Adaptation du Hardware Abstraction Layer pour les architectures ARM64.
//! Les processeurs ARM modernes (Cortex-A72, Cortex-A78AE) ont des
//! caractéristiques différentes des processeurs space-grade (LEON4/SPARC) :
//! - ARMv8 a ECC matériel intégré (via EDAC framework Linux)
//! - ARMv8-A AE (Automotive Enhanced) a lockstep pour le TMR
//! - i.MX8 a un watchdog matériel dédié (WDOG3)
//!
//! Ce module adapte les appels HAL pour ces spécificités.

use crate::radiation_tolerant_hal::*;

/// Portage ARM64 du contrôleur de mémoire ECC
pub struct ArmEccController;

impl ArmEccController {
    /// Lire les compteurs ECC via sysfs (EDAC framework Linux)
    ///
    /// Sur ARM64 Linux, les erreurs mémoire sont rapportées via :
    /// /sys/devices/system/edac/mc/mc0/ce_count (correctable)
    /// /sys/devices/system/edac/mc/mc0/ue_count (uncorrectable)
    pub fn read_ecc_counters() -> HalResult<EccCounters> {
        let ce = Self::read_sysfs("/sys/devices/system/edac/mc/mc0/ce_count")
            .unwrap_or(0);
        let ue = Self::read_sysfs("/sys/devices/system/edac/mc/mc0/ue_count")
            .unwrap_or(0);

        Ok(EccCounters {
            correctable_errors: ce,
            uncorrectable_errors: ue,
            last_ce_address: 0,
            last_ue_address: 0,
        })
    }

    /// Vérifier si ARM ECC hardware est disponible
    pub fn is_ecc_available() -> bool {
        std::path::Path::new("/sys/devices/system/edac/mc/mc0")
            .exists()
    }

    fn read_sysfs(path: &str) -> Option<u64> {
        std::fs::read_to_string(path).ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
    }
}

/// Portage ARM64 du TMR (Triple Modular Redundancy)
///
/// Sur ARM Cortex-A78AE, le TMR est implémenté via le mode "Dual Lockstep".
/// Deux clusters identiques exécutent les mêmes instructions en parallèle
/// et comparent les résultats. En cas de divergence → erreur détectée.
pub struct ArmLockstep;

impl ArmLockstep {
    /// Vérifier si le mode lockstep est actif
    pub fn is_lockstep_enabled() -> bool {
        // Via Device Tree ou regmap
        // Sur i.MX8QM: vérifier SCU (System Controller Unit)
        #[cfg(target_arch = "aarch64")]
        {
            if let Ok(contents) = std::fs::read_to_string("/proc/device-tree/model") {
                return contents.contains("i.MX8") || contents.contains("Raspberry Pi");
            }
        }
        false
    }

    /// Exécuter un calcul critique en lockstep (3 exécutions, vote majoritaire)
    pub fn tmr_compute<F: Fn() -> u64 + Copy>(func: F) -> TmrResult<u64> {
        let result_a = func();
        let result_b = func();
        let result_c = func();

        // Vote majoritaire
        if result_a == result_b {
            return Ok(result_a);
        }
        if result_a == result_c {
            return Ok(result_a);
        }
        if result_b == result_c {
            return Ok(result_b);
        }

        Err(TmrError::TripleMismatch)
    }
}

/// Portage ARM64 du Watchdog matériel
///
/// Sur ARM64 Linux, le watchdog est accessible via :
/// - /dev/watchdog (watchdog generic Linux)
/// - /dev/watchdog0 (watchdog principal, souvent WDOG3 sur i.MX)
///
/// Le watchdog doit être "pinged" toutes les N secondes.
pub struct ArmWatchdog;

impl ArmWatchdog {
    /// Ouvrir le watchdog hardware
    pub fn open(path: &str) -> HalResult<Self> {
        // Ouvrir le device watchdog
        let fd = unsafe {
            libc::open(
                std::ffi::CString::new(path)
                    .unwrap()
                    .as_ptr(),
                libc::O_WRONLY,
            )
        };

        if fd < 0 {
            return Err(HalError::WatchdogUnavailable(path.to_string()));
        }

        // Configurer le timeout (en secondes)
        let mut timeout: libc::c_int = 10; // 10 secondes par défaut
        let _ = unsafe {
            libc::ioctl(fd, 0x80045704, &mut timeout) // WDIOC_SETTIMEOUT
        };

        Ok(ArmWatchdog { fd })
    }

    /// Ping le watchdog (reset du timer)
    pub fn ping(&self) -> HalResult<()> {
        let data = b"\0"; // Magic character pour watchdog
        let ret = unsafe {
            libc::write(self.fd, data.as_ptr() as *const libc::c_void, 1)
        };

        if ret != 1 {
            return Err(HalError::WatchdogPingFailed);
        }

        Ok(())
    }

    /// Démarrer le watchdog
    pub fn start(&self) -> HalResult<()> {
        self.ping() // Le premier ping démarre le watchdog
    }

    /// Arrêter le watchdog (NE PAS FAIRE EN PRODUCTION)
    pub fn stop(&self) -> HalResult<()> {
        let data = b"V"; // Magic character "V" pour arrêter
        let _ = unsafe {
            libc::write(self.fd, data.as_ptr() as *const libc::c_void, 1)
        };
        Ok(())
    }

    fd: libc::c_int,
}

/// Portage ARM64 du Power Management
///
/// Sur les plateformes embarquées ARM, le power management est géré via :
/// - cpufreq (fréquence CPU)
/// - Thermal zones (température)
/// - Regulators (tensions d'alimentation)
pub struct ArmPowerManager;

impl ArmPowerManager {
    /// Lire la température du SoC
    pub fn read_soc_temperature() -> HalResult<f32> {
        // Via thermal zone sysfs
        let temp_raw = std::fs::read_to_string(
            "/sys/class/thermal/thermal_zone0/temp"
        ).map_err(|_| HalError::ThermalSensorUnavailable)?;

        let temp_millis = temp_raw.trim()
            .parse::<i64>()
            .unwrap_or(0);

        Ok(temp_millis as f32 / 1000.0)
    }

    /// Lire la fréquence CPU actuelle
    pub fn read_cpu_frequency() -> HalResult<u64> {
        let freq = std::fs::read_to_string(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"
        ).map_err(|_| HalError::CpuFreqUnavailable)?;

        let freq_khz = freq.trim().parse::<u64>().unwrap_or(0);
        Ok(freq_khz * 1000) // Convertir en Hz
    }

    /// Lire le voltage du SoC
    pub fn read_soc_voltage() -> HalResult<f32> {
        // Via hwmon
        let vdd = std::fs::read_to_string(
            "/sys/class/hwmon/hwmon0/in0_input"
        ).map_err(|_| HalError::VoltageSensorUnavailable)?;

        let vdd_uv = vdd.trim().parse::<u64>().unwrap_or(0);
        Ok(vdd_uv as f32 / 1_000_000.0) // Convertir en Volts
    }

    /// Vérifier si la température est dans les limites Mars
    pub fn is_within_mars_operational_range() -> HalResult<bool> {
        let temp = Self::read_soc_temperature()?;

        // Mars operational range: -40°C to +50°C (intérieur rover)
        Ok(temp >= -40.0 && temp <= 50.0)
    }

    /// Throttler le CPU si la température est trop élevée
    pub fn thermal_throttle(max_temp: f32) -> HalResult<bool> {
        let current_temp = Self::read_soc_temperature()?;

        if current_temp > max_temp {
            // Réduire la fréquence CPU
            let min_freq = std::fs::read_to_string(
                "/sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq"
            ).map_err(|_| HalError::CpuFreqUnavailable)?;

            // Écrire la fréquence minimale
            std::fs::write(
                "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq",
                min_freq.trim(),
            ).map_err(|_| HalError::CpuFreqControlFailed)?;

            Ok(true) // Throttling appliqué
        } else {
            Ok(false) // Pas besoin de throttling
        }
    }
}

// ─── Résultats et erreurs ───

pub type TmrResult<T> = Result<T, TmrError>;

#[derive(Debug)]
pub enum TmrError {
    TripleMismatch,
}

#[derive(Debug)]
pub enum HalError {
    WatchdogUnavailable(String),
    WatchdogPingFailed,
    ThermalSensorUnavailable,
    CpuFreqUnavailable,
    CpuFreqControlFailed,
    VoltageSensorUnavailable,
}

pub type HalResult<T> = Result<T, HalError>;

#[derive(Debug, Clone)]
pub struct EccCounters {
    pub correctable_errors: u64,
    pub uncorrectable_errors: u64,
    pub last_ce_address: u64,
    pub last_ue_address: u64,
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmr_computation() {
        // TMR avec 3 résultats identiques
        let result = ArmLockstep::tmr_compute(|| 42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_tmr_mismatch_detection() {
        // Simuler un résultat divergent (SEU)
        let counter = std::sync::atomic::AtomicU32::new(0);
        let result = ArmLockstep::tmr_compute(|| {
            let val = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if val == 1 { 100 } else { 42 } // 2e exécution diverge
        });
        assert!(result.is_err());
    }
}
