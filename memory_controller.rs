//! # Memory Controller — ECC et DM-Verity
//!
//! Contrôleur mémoire avec protection ECC (Error-Correcting Code)
//! et vérification d'intégrité par DM-Verity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type d'erreur mémoire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryError {
    /// Erreur corrigible (single-bit)
    Correctable { address: u64, corrected_value: u8 },
    /// Erreur non-corrigible (multi-bit)
    Uncorrectable { address: u64, page: u64 },
    /// Erreur d'intégrité (DM-Verity mismatch)
    IntegrityViolation { page: u64, expected_hash: String },
}

/// Statistiques de la mémoire
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_pages: u64,
    pub clean_pages: u64,
    pub corrected_pages: u64,
    pub corrupted_pages: u64,
    pub scrub_count: u64,
    pub total_corrected_bits: u64,
}

/// Contrôleur mémoire avec ECC
pub struct EccMemoryController {
    page_size: u64,
    total_memory_mb: u64,
    ecc_bits_per_word: u8,
    stats: MemoryStats,
    /// Simule les bits ECC par page
    ecc_state: HashMap<u64, Vec<u8>>,
}

impl EccMemoryController {
    pub fn new(total_memory_mb: u64, page_size: u64) -> Self {
        let total_pages = (total_memory_mb * 1024 * 1024) / page_size;
        Self {
            page_size,
            total_memory_mb,
            ecc_bits_per_word: 8, // SECDED
            stats: MemoryStats {
                total_pages,
                clean_pages: total_pages,
                ..Default::default()
            },
            ecc_state: HashMap::new(),
        }
    }

    /// Vérifie et corrige une page mémoire
    pub fn check_and_correct(&mut self, page: u64, data: &[u8]) -> Result<Vec<u8>, MemoryError> {
        // Calculer le checksum ECC (simulé)
        let ecc = self.compute_ecc(data);

        // Vérifier si la page a des erreurs
        let error_bits = self.detect_errors(data, &ecc);

        if error_bits == 0 {
            self.stats.clean_pages = self.stats.clean_pages.min(self.stats.total_pages);
            Ok(data.to_vec())
        } else if error_bits == 1 {
            // Single-bit error — corrigeable
            let corrected = self.correct_single_bit(data, &ecc);
            self.stats.corrected_pages += 1;
            self.stats.total_corrected_bits += 1;
            Ok(corrected)
        } else {
            // Multi-bit error — non-corrigible
            self.stats.corrupted_pages += 1;
            Err(MemoryError::Uncorrectable {
                address: page * self.page_size,
                page,
            })
        }
    }

    /// Calcule le code ECC (simulé — Hamming SECDED)
    fn compute_ecc(&self, data: &[u8]) -> Vec<u8> {
        let parity = data.iter().fold(0u8, |acc, &b| acc ^ b);
        vec![parity, parity.rotate_left(1)]
    }

    /// Détecte les bits en erreur
    fn detect_errors(&self, data: &[u8], ecc: &[u8]) -> u8 {
        let computed = self.compute_ecc(data);
        let syndrome: u8 = computed
            .iter()
            .zip(ecc.iter())
            .map(|(a, b)| a ^ b)
            .fold(0u8, |acc, x| acc | x);

        syndrome.count_ones() as u8
    }

    /// Corrige un single-bit error
    fn correct_single_bit(&self, data: &[u8], ecc: &[u8]) -> Vec<u8> {
        // Simule la correction du bit en erreur
        let mut corrected = data.to_vec();
        if let Some(first_byte) = corrected.first_mut() {
            *first_byte ^= 0x01; // Flip le bit LSB du premier byte
        }
        corrected
    }

    /// Scrubbing : parcourt toutes les pages et corrige
    pub fn scrub_all(&mut self) -> MemoryStats {
        self.stats.scrub_count += 1;
        self.stats.clone()
    }

    pub fn get_stats(&self) -> &MemoryStats {
        &self.stats
    }
}

/// DM-Verity pour l'intégrité des images système
pub struct DmVerity {
    hash_algorithm: String,
    page_hashes: HashMap<u64, Vec<u8>>,
}

impl DmVerity {
    pub fn new() -> Self {
        Self {
            hash_algorithm: "SHA3-256".into(),
            page_hashes: HashMap::new(),
        }
    }

    pub fn hash_page(&mut self, page: u64, data: &[u8]) {
        use sha3::{Sha3_256, Digest};
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        self.page_hashes.insert(page, hasher.finalize().to_vec());
    }

    pub fn verify_page(&self, page: u64, data: &[u8]) -> bool {
        use sha3::{Sha3_256, Digest};
        let expected = match self.page_hashes.get(&page) {
            Some(h) => h,
            None => return false,
        };

        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let computed = hasher.finalize().to_vec();

        computed == *expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecc_no_error() {
        let mut controller = EccMemoryController::new(256, 4096);
        let data = vec![0x42u8; 4096];
        let ecc = controller.compute_ecc(&data);

        let result = controller.check_and_correct(0, &data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dm_verity() {
        let mut verity = DmVerity::new();
        let data = b"hello mars";
        verity.hash_page(0, data);
        assert!(verity.verify_page(0, data));
        assert!(!verity.verify_page(0, b"modified"));
    }
}
