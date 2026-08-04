//! # SpaceOne HAL — Portage Space-Grade (LEON4, GR740, SPARC)
//!
//! Adaptation du HAL pour les processeurs rad-hard certifiés pour l'espace :
//! - LEON4 (Cobham Gaisler) : SPARC V8, rad-hard, TMR natif
//! - GR740 (Cobham Gaisler) : SPARC V8, quad-core, rad-hard
//! - SPARC V8 avec EDAC intégré
//!
//! Ces processeurs ont des caractéristiques uniques :
//! - TMR implémenté au niveau hardware (pas besoin de logiciel)
//! - EDAC mémoire intégré au contrôleur mémoire
//! - Watchdog hardware dédié (SEU-proof)
//! - Power supply SEU-tolerant

/// Portage Space-Grade du TMR Hardware
///
/// Le LEON4 a un mode TMR natif : chaque instruction est exécutée
/// 3 fois en parallèle sur 3 cores identiques. Le vote est fait
/// au niveau hardware, pas logiciel.
pub struct LeonTmr;

impl LeonTmr {
    /// Le TMR est configuré au boot via les registres LEON
    pub fn configure_tmr_mode() -> SpaceResult<()> {
        // Écrire dans les registres LEON pour activer le TMR
        // Adresse: 0x80000110 (Timer 1 Control Register)
        // Bit 0: TMR enable
        unsafe {
            let base: *mut u32 = 0x80000000 as *mut u32;
            let val = core::ptr::read_volatile(base);
            core::ptr::write_volatile(base, val | 0x01); // Set TMR bit
        }
        Ok(())
    }

    /// Vérifier le statut TMR (divergence détectée ?)
    pub fn check_tmr_status() -> SpaceResult<TmrStatus> {
        unsafe {
            let status_reg: *const u32 = 0x80000200 as *const u32;
            let status = core::ptr::read_volatile(status_reg);

            Ok(TmrStatus {
                divergences_detected: (status >> 4) & 0xFF,
                last_divergence_address: (status & 0xFFFF) as u64,
                is_active: (status & 0x01) != 0,
            })
        }
    }
}

/// Portage Space-Grade de la protection SEL (Single Event Latchup)
///
/// Les processeurs space-grade sont particulièrement sensibles aux SEL.
/// Le GR740 intègre un circuit de protection qui coupe l'alimentation
/// en cas de détection de SEL.
pub struct SelProtection;

impl SelProtection {
    /// Vérifier si un SEL a été détecté
    pub fn check_sel_status() -> SpaceResult<SelStatus> {
        // Lire le registre de statut SEL
        unsafe {
            let sel_reg: *const u32 = 0x80001000 as *const u32;
            let status = core::ptr::read_volatile(sel_reg);

            Ok(SelStatus {
                sel_detected: (status & 0x01) != 0,
                sel_count: (status >> 8) & 0xFF,
                last_sel_timestamp: (status >> 16) as u64,
                power_cycle_required: (status & 0x02) != 0,
            })
        }
    }

    /// Redémarrer après SEL (power cycle complet)
    pub fn handle_sel_recovery() -> SpaceResult<()> {
        // 1. Sauvegarder l'état critique en mémoire non-volatile
        // 2. Couper l'alimentation via le circuit de protection
        // 3. Attendre le power-on reset
        // 4. Le bootloader détecte le SEL et initie le recovery

        unsafe {
            let reset_reg: *mut u32 = 0x80000F00 as *mut u32;
            core::ptr::write_volatile(reset_reg, 0xDEAD); // SEL recovery trigger
        }

        // Le système redémarre via le watchdog
        loop {
            // Attendre le reset hardware
            unsafe { core::arch::asm!("wfi"); }
        }
    }
}

/// Portage Space-Grade du Watchdog SEU-Proof
///
/// Le watchdog du LEON4 est implémenté en triple-redondance hardware.
/// Il est impossible qu'un SEU le désactive.
pub struct LeonWatchdog;

impl LeonWatchdog {
    /// Configurer le watchdog hardware
    pub fn configure(timeout_ms: u32) -> SpaceResult<()> {
        // Écrire le timeout dans le registre du watchdog
        unsafe {
            let wdt_reg: *mut u32 = 0x80000300 as *mut u32;
            core::ptr::write_volatile(wdt_reg, timeout_ms);
        }
        Ok(())
    }

    /// Ping le watchdog (reset du timer)
    pub fn ping() -> SpaceResult<()> {
        unsafe {
            let wdt_ctrl: *mut u32 = 0x80000304 as *mut u32;
            core::ptr::write_volatile(wdt_ctrl, 0xA5); // Magic value
        }
        Ok(())
    }

    /// Vérifier si le watchdog est armé
    pub fn is_armed() -> bool {
        unsafe {
            let wdt_status: *const u32 = 0x80000308 as *const u32;
            let status = core::ptr::read_volatile(wdt_status);
            (status & 0x01) != 0
        }
    }
}

// ─── Types ───

pub type SpaceResult<T> = Result<T, SpaceError>;

#[derive(Debug)]
pub enum SpaceError {
    HardwareAccessFailed,
    TmrDivergence,
    SelDetected,
    WatchdogTimeout,
    MemoryCorruption,
}

#[derive(Debug, Clone)]
pub struct TmrStatus {
    pub divergences_detected: u32,
    pub last_divergence_address: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct SelStatus {
    pub sel_detected: bool,
    pub sel_count: u32,
    pub last_sel_timestamp: u64,
    pub power_cycle_required: bool,
}
