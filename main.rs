//! # Data Processor — Analyse des Données Science
//!
//! Traite les données scientifiques reçues de Mars, décompresse
//! les bundles DTN++, et prépare les données pour l'analyse
//! par les scientifiques et le stockage dans les archives.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use chrono::{DateTime, Utc};
use sha3::{Sha3_256, Digest};
use tracing::{info, warn};

/// Type de donnée scientifique
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScienceDataType {
    Image,
    Spectrometer,
    DrillSample,
    WeatherStation,
    Seismometer,
    Magnetometer,
    AtmosphericPressure,
    RadiationDose,
}

/// Package de données scientifiques
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScienceDataPackage {
    pub id: String,
    pub data_type: ScienceDataType,
    pub source_asset: String,
    pub timestamp: DateTime<Utc>,
    pub sol_number: u32,
    pub compressed_size_bytes: u64,
    pub uncompressed_size_bytes: u64,
    pub compression_ratio: f64,
    pub hash: String,
    pub metadata: HashMap<String, String>,
    pub priority: u8,
}

/// Résultat du traitement
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingResult {
    pub package_id: String,
    pub status: ProcessingStatus,
    pub processing_time_ms: u64,
    pub output_path: String,
    pub quality_score: f64,
    pub anomalies_detected: Vec<String>,
}

/// Statut de traitement
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProcessingStatus {
    Received,
    Decompressing,
    Analyzing,
    Completed,
    Failed { reason: String },
}

/// Processeur de données
pub struct DataProcessor {
    processed_packages: Vec<ProcessingResult>,
    total_bytes_processed: u64,
    compression_stats: CompressionStats,
}

/// Statistiques de compression
#[derive(Debug, Clone, Default, Serialize)]
pub struct CompressionStats {
    pub total_compressed_bytes: u64,
    pub total_uncompressed_bytes: u64,
    pub average_ratio: f64,
    pub best_ratio: f64,
    pub worst_ratio: f64,
    pub packages_processed: u32,
}

impl DataProcessor {
    pub fn new() -> Self {
        Self {
            processed_packages: Vec::new(),
            total_bytes_processed: 0,
            compression_stats: CompressionStats::default(),
        }
    }

    /// Traite un package de données reçu
    pub fn process_package(&mut self, package: ScienceDataPackage, raw_data: &[u8]) -> ProcessingResult {
        let start = std::time::Instant::now();

        // Étape 1: Vérification d'intégrité
        let mut hasher = Sha3_256::new();
        hasher.update(raw_data);
        let computed_hash = hex::encode(hasher.finalize());

        if computed_hash != package.hash {
            return ProcessingResult {
                package_id: package.id.clone(),
                status: ProcessingStatus::Failed { reason: "Hash mismatch".into() },
                processing_time_ms: start.elapsed().as_millis() as u64,
                output_path: String::new(),
                quality_score: 0.0,
                anomalies_detected: vec!["Data integrity compromised".into()],
            };
        }

        // Étape 2: Décompression (simulation)
        let _decompressed = raw_data; // En production: decompress AI-compressed data

        // Étape 3: Analyse qualité
        let quality = self.analyze_quality(&package, raw_data);

        // Étape 4: Détection d'anomalies
        let anomalies = self.detect_anomalies(&package, raw_data);

        // Étape 5: Stockage
        let output_path = format!("/data/science/{}/{}", package.data_type, package.id);

        // Mettre à jour les stats
        self.total_bytes_processed += package.uncompressed_size_bytes;
        self.compression_stats.packages_processed += 1;
        self.compression_stats.total_compressed_bytes += package.compressed_size_bytes;
        self.compression_stats.total_uncompressed_bytes += package.uncompressed_size_bytes;

        if package.compression_ratio > self.compression_stats.best_ratio {
            self.compression_stats.best_ratio = package.compression_ratio;
        }
        if package.compression_ratio < self.compression_stats.worst_ratio || self.compression_stats.worst_ratio == 0.0 {
            self.compression_stats.worst_ratio = package.compression_ratio;
        }

        self.compression_stats.average_ratio =
            self.compression_stats.total_uncompressed_bytes as f64 /
            self.compression_stats.total_compressed_bytes.max(1) as f64;

        self.processed_packages.push(ProcessingResult {
            package_id: package.id.clone(),
            status: ProcessingStatus::Completed,
            processing_time_ms: start.elapsed().as_millis() as u64,
            output_path: output_path.clone(),
            quality_score: quality,
            anomalies_detected: anomalies.clone(),
        });

        ProcessingResult {
            package_id: package.id,
            status: ProcessingStatus::Completed,
            processing_time_ms: start.elapsed().as_millis() as u64,
            output_path,
            quality_score: quality,
            anomalies_detected: anomalies,
        }
    }

    /// Analyse la qualité des données
    fn analyze_quality(&self, package: &ScienceDataPackage, data: &[u8]) -> f64 {
        let mut score = 100.0;

        // Pénalité pour données courtes
        if data.len() < 100 {
            score -= 20.0;
        }

        // Pénalité pour metadata manquante
        if package.metadata.is_empty() {
            score -= 10.0;
        }

        // Bonus pour compression élevée (données riches)
        if package.compression_ratio > 10.0 {
            score += 5.0;
        }

        score.min(100.0).max(0.0)
    }

    /// Détecte les anomalies dans les données
    fn detect_anomalies(&self, package: &ScienceDataPackage, data: &[u8]) -> Vec<String> {
        let mut anomalies = Vec::new();

        // Vérifier les valeurs aberrantes
        if data.iter().all(|&b| b == 0) {
            anomalies.push("All-zero data detected — possible sensor failure".into());
        }

        if data.iter().all(|&b| b == 255) {
            anomalies.push("All-255 data detected — possible saturation".into());
        }

        // Vérifier la cohérence temporelle
        let age = Utc::now() - package.timestamp;
        if age.num_days() > 365 {
            anomalies.push(format!("Data age: {} days — possible timestamp error", age.num_days()));
        }

        anomalies
    }

    /// Statistiques du processeur
    pub fn get_stats(&self) -> &CompressionStats {
        &self.compression_stats
    }

    /// Résultats de traitement
    pub fn results(&self) -> &[ProcessingResult] {
        &self.processed_packages
    }

    /// Total de données traitées
    pub fn total_processed_mb(&self) -> f64 {
        self.total_bytes_processed as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_process_package() {
        let mut processor = DataProcessor::new();

        let data = vec![42u8; 10000];
        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let hash = hex::encode(hasher.finalize());

        let package = ScienceDataPackage {
            id: "sci-001".into(),
            data_type: ScienceDataType::Image,
            source_asset: "rover-01".into(),
            timestamp: Utc::now(),
            sol_number: 45,
            compressed_size_bytes: 2500,
            uncompressed_size_bytes: 10000,
            compression_ratio: 4.0,
            hash,
            metadata: HashMap::new(),
            priority: 5,
        };

        let result = processor.process_package(package, &data);
        assert_eq!(result.status, ProcessingStatus::Completed);
        assert!(result.quality_score > 0.0);
    }

    #[test]
    fn test_integrity_check_fails() {
        let mut processor = DataProcessor::new();

        let package = ScienceDataPackage {
            id: "sci-002".into(),
            data_type: ScienceDataType::Spectrometer,
            source_asset: "rover-01".into(),
            timestamp: Utc::now(),
            sol_number: 45,
            compressed_size_bytes: 100,
            uncompressed_size_bytes: 500,
            compression_ratio: 5.0,
            hash: "wrong_hash".into(),
            metadata: HashMap::new(),
            priority: 3,
        };

        let result = processor.process_package(package, &[1, 2, 3]);
        assert!(matches!(result.status, ProcessingStatus::Failed { .. }));
    }
}
