//! # Science Data Pipeline — Traitement des données scientifiques Mars
//!
//! Reçoit les bundles DTN contenant les données scientifiques,
//! les décompresse, valide, classe, et les rend disponibles pour
//! les scientifiques via l'API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

// ─── Modèles ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScienceDataRecord {
    pub id: String,
    pub instrument: InstrumentType,
    pub source_asset: String,
    pub sol_number: u32,
    pub timestamp: DateTime<Utc>,
    pub data_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub compression_ratio: f64,
    pub sha3_256: String,
    pub quality_score: f32,
    pub classification: DataClassification,
    pub metadata: HashMap<String, String>,
    pub anomalies: Vec<String>,
    pub available_on_mars: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum InstrumentType {
    MastCamera,
    SuperCam,
    PiXL,
    SHERLOC,
    MOXIE,
    MEDA,
    Seismometer,
    Magnetometer,
    WeatherStation,
    DrillSample,
    AtmosphericPressure,
    RadiationDose,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    ScientificSecret,
}

/// Pipeline de traitement
pub struct SciencePipeline {
    records: Vec<ScienceDataRecord>,
    total_bytes_received: u64,
    total_anomalies: u32,
    classification_counts: HashMap<String, u32>,
}

impl SciencePipeline {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            total_bytes_received: 0,
            total_anomalies: 0,
            classification_counts: HashMap::new(),
        }
    }

    /// Traiter un package de données reçu via DTN
    pub fn ingest(&mut self, data: &[u8], instrument: InstrumentType, source: &str, sol: u32) -> Result<String, String> {
        // 1. Hash de vérification
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());

        // 2. Estimation compression (en production: décompression réelle)
        let original_size = data.len() as u64;
        let compressed_estimate = (original_size as f64 * 0.4) as u64; // ~2.5x compression
        let ratio = if compressed_estimate > 0 {
            original_size as f64 / compressed_estimate as f64
        } else {
            1.0
        };

        // 3. Détection d'anomalies
        let anomalies = Self::detect_anomalies(data, &instrument);

        // 4. Classification
        let classification = Self::classify_data(&instrument, &anomalies);

        // 5. Score qualité
        let quality = Self::compute_quality(data, &anomalies);

        let record = ScienceDataRecord {
            id: format!("SCI-{}", Uuid::new_v4().to_string().split('-').next().unwrap()),
            instrument,
            source_asset: source.to_string(),
            sol_number: sol,
            timestamp: Utc::now(),
            data_size_bytes: original_size,
            compressed_size_bytes: compressed_estimate,
            compression_ratio: ratio,
            sha3_256: hash,
            quality_score: quality,
            classification: classification.clone(),
            metadata: HashMap::new(),
            anomalies: anomalies.clone(),
            available_on_mars: false,
        };

        self.total_bytes_received += original_size;
        self.total_anomalies += anomalies.len() as u32;
        *self.classification_counts.entry(format!("{:?}", classification)).or_insert(0) += 1;

        let id = record.id.clone();
        info!("Science data ingested: {} ({} bytes, quality: {:.1}, anomalies: {})",
              id, original_size, quality, anomalies.len());

        self.records.push(record);
        Ok(id)
    }

    /// Détecter les anomalies dans les données
    fn detect_anomalies(data: &[u8], instrument: &InstrumentType) -> Vec<String> {
        let mut anomalies = Vec::new();

        // Données toutes nulles
        if data.iter().all(|&b| b == 0) && data.len() > 100 {
            anomalies.push("All-zero data — possible sensor failure".into());
        }

        // Données saturées
        if data.iter().all(|&b| b == 255) && data.len() > 100 {
            anomalies.push("All-saturated data — possible sensor overload".into());
        }

        // Données très courtes
        if data.len() < 10 {
            anomalies.push("Data too short — possible transmission error".into());
        }

        // Vérifier la distribution (détection de bruit)
        if data.len() > 1000 {
            let unique_values: std::collections::HashSet<u8> = data.iter().cloned().collect();
            if unique_values.len() < 3 {
                anomalies.push(format!("Low entropy: only {} unique values in {} bytes",
                    unique_values.len(), data.len()));
            }
        }

        anomalies
    }

    /// Classer les données
    fn classify_data(instrument: &InstrumentType, anomalies: &[String]) -> DataClassification {
        match instrument {
            InstrumentType::DrillSample => DataClassification::ScientificSecret,
            InstrumentType::SHERLOC => DataClassification::ScientificSecret,
            InstrumentType::SuperCam => DataClassification::Confidential,
            InstrumentType::PiXL => DataClassification::Confidential,
            _ if !anomalies.is_empty() => DataClassification::Internal,
            _ => DataClassification::Public,
        }
    }

    /// Calculer le score qualité
    fn compute_quality(data: &[u8], anomalies: &[String]) -> f32 {
        let mut score = 100.0;

        if !anomalies.is_empty() {
            score -= (anomalies.len() as f32 * 15.0);
        }

        if data.len() < 100 {
            score -= 30.0;
        }

        if data.len() > 10000 {
            score += 5.0; // Bonus pour données riches
        }

        score.max(0.0).min(100.0)
    }

    /// Statistiques du pipeline
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            total_records: self.records.len(),
            total_bytes_gb: self.total_bytes_received as f64 / (1024.0 * 1024.0 * 1024.0),
            total_anomalies: self.total_anomalies,
            average_quality: if self.records.is_empty() {
                0.0
            } else {
                self.records.iter().map(|r| r.quality_score).sum::<f32>() / self.records.len() as f32
            },
            by_instrument: self.count_by_instrument(),
            by_classification: self.classification_counts.clone(),
        }
    }

    /// Récupérer les records par sol
    pub fn records_by_sol(&self, sol: u32) -> Vec<&ScienceDataRecord> {
        self.records.iter().filter(|r| r.sol_number == sol).collect()
    }

    /// Récupérer les records par instrument
    pub fn records_by_instrument(&self, instrument: &InstrumentType) -> Vec<&ScienceDataRecord> {
        self.records.iter().filter(|r| &r.instrument == instrument).collect()
    }

    /// Récupérer les records avec anomalies
    pub fn anomalous_records(&self) -> Vec<&ScienceDataRecord> {
        self.records.iter().filter(|r| !r.anomalies.is_empty()).collect()
    }

    fn count_by_instrument(&self) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        for record in &self.records {
            *counts.entry(format!("{:?}", record.instrument)).or_insert(0) += 1;
        }
        counts
    }
}

/// Statistiques du pipeline
#[derive(Clone, Debug, Serialize)]
pub struct PipelineStats {
    pub total_records: usize,
    pub total_bytes_gb: f64,
    pub total_anomalies: u32,
    pub average_quality: f32,
    pub by_instrument: HashMap<String, u32>,
    pub by_classification: HashMap<String, u32>,
}
