//! # AI Compression — Compression Adaptative par Réseau Neuronal
//!
//! Module de compression intelligent qui s'adapte au type de données
//! et à la bande passante disponible. Sur Mars, chaque bit compte.
//!
//! ## Stratégies
//!
//! - **Lossless** : Pour les commandes et données safety-critical
//! - **Near-lossless** : Pour les images scientifiques (JPEG2000-like)
//! - **Lossy with ROI** : Region-of-interest pour les panoramas
//! - **Semantic** : Extraction de features pour la télémesure

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};

/// Erreurs de compression
#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Taille d'entrée invalide: {0} octets")]
    InvalidInputSize(usize),

    #[error("Algorithme non supporté: {0}")]
    UnsupportedAlgorithm(String),

    #[error("Ratio de compression insuffisant: {0:.2}x (min={1:.2}x)")]
    InsufficientCompression(f64, f64),

    #[error("Données corrompues après décompression")]
    DecompressionCorruption,
}

pub type CompressionResult<T> = Result<T, CompressionError>;

/// Type de contenu à compresser
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    /// Données binaires critiques (commandes, configs)
    Binary,
    /// Texte structuré (JSON, XML, CSV)
    Text,
    /// Image scientifique (RAW, TIFF)
    ScientificImage,
    /// Panorama ou photo couleur
    Photo,
    /// Vidéo compressée
    Video,
    /// Télémesure (timeseries)
    Telemetry,
    /// Audio (communications équipage)
    Audio,
}

/// Stratégie de compression sélectionnée
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionStrategy {
    /// Pas de compression (données déjà optimales)
    None,
    /// Compression sans perte (LZ4, Zstd)
    Lossless,
    /// Compression quasi-sans perte (tolérance fixée)
    NearLossless { max_error: f64 },
    /// Compression avec perte (focus sur ROI)
    LossyWithRoi { quality: f64, roi_pct: f64 },
    /// Extraction sémantique (features only)
    Semantic { feature_count: u16 },
}

/// Résultat d'une opération de compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult2 {
    pub original_size: u64,
    pub compressed_size: u64,
    pub ratio: f64,
    pub strategy: CompressionStrategy,
    pub algorithm: String,
    pub compression_time_ms: u64,
    pub quality_preserved: f64, // 0.0-1.0
}

/// Contexte de bande passante pour la sélection de stratégie
#[derive(Debug, Clone)]
pub struct BandwidthContext {
    pub available_kbps: f64,
    pub latency_minutes: f64,
    pub contact_duration_secs: u32,
    pub content_type: ContentType,
    pub original_size_bytes: u64,
}

/// Le compresseur AI adaptatif
pub struct AiCompressor {
    /// Ratio de compression cible minimum
    min_ratio: f64,
    /// Taille maximale d'un bundle après compression (octets)
    max_bundle_size: u64,
    /// Cache de statistiques pour l'adaptation
    stats: CompressionStats,
}

/// Statistiques de compression pour l'adaptation
#[derive(Debug, Clone, Default)]
struct CompressionStats {
    total_compressed: u64,
    total_original: u64,
    strategies_used: std::collections::HashMap<String, u64>,
}

impl AiCompressor {
    /// Crée un nouveau compresseur avec paramètres par défaut
    pub fn new() -> Self {
        Self {
            min_ratio: 1.5,
            max_bundle_size: 1024 * 1024, // 1 MB max per bundle
            stats: CompressionStats::default(),
        }
    }

    /// Crée un compresseur avec paramètres personnalisés
    pub fn with_config(min_ratio: f64, max_bundle_size: u64) -> Self {
        Self {
            min_ratio,
            max_bundle_size,
            stats: CompressionStats::default(),
        }
    }

    /// Sélectionne la meilleure stratégie de compression pour le contexte
    pub fn select_strategy(&self, context: &BandwidthContext) -> CompressionStrategy {
        // Calculer le ratio nécessaire
        let max_time = context.contact_duration_secs as f64;
        let max_bytes = context.available_kbps * 1000.0 * max_time / 8.0;

        if context.original_size_bytes as f64 <= max_bytes {
            return CompressionStrategy::None; // Pas besoin de compression
        }

        let required_ratio = context.original_size_bytes as f64 / max_bytes;

        // Sélection basée sur le type de contenu et le ratio requis
        match context.content_type {
            ContentType::Binary => CompressionStrategy::Lossless,
            ContentType::Text => CompressionStrategy::Lossless,
            ContentType::ScientificImage => {
                if required_ratio < 3.0 {
                    CompressionStrategy::NearLossless { max_error: 0.01 }
                } else {
                    CompressionStrategy::LossyWithRoi {
                        quality: 0.7,
                        roi_pct: 0.3,
                    }
                }
            }
            ContentType::Photo => {
                CompressionStrategy::LossyWithRoi {
                    quality: 0.5,
                    roi_pct: 0.2,
                }
            }
            ContentType::Video => CompressionStrategy::Semantic { feature_count: 256 },
            ContentType::Telemetry => CompressionStrategy::Lossless,
            ContentType::Audio => {
                if required_ratio < 4.0 {
                    CompressionStrategy::NearLossless { max_error: 0.05 }
                } else {
                    CompressionStrategy::LossyWithRoi {
                        quality: 0.6,
                        roi_pct: 0.8, // Audio: presque tout est ROI
                    }
                }
            }
        }
    }

    /// Comprime des données avec la stratégie donnée (simulation)
    ///
    /// En production, ce module appelle un modèle ONNX embarqué.
    /// Ici, on simule les ratios de compression typiques.
    pub fn compress(
        &self,
        data: &[u8],
        strategy: &CompressionStrategy,
    ) -> CompressionResult<CompressionResult2> {
        let start = std::time::Instant::now();
        let original_size = data.len() as u64;

        if original_size == 0 {
            return Err(CompressionError::InvalidInputSize(0));
        }

        let (compressed_size, algorithm, quality) = match strategy {
            CompressionStrategy::None => {
                (original_size, "none".into(), 1.0)
            }
            CompressionStrategy::Lossless => {
                // Zstd-like: ratio ~2-3x pour données binaires/texte
                let ratio = if data.len() > 1000 { 2.5 } else { 1.8 };
                let size = (original_size as f64 / ratio) as u64;
                (size, "zstd".into(), 1.0)
            }
            CompressionStrategy::NearLossless { max_error } => {
                // Ratio ~4-6x avec tolérance d'erreur
                let ratio = 5.0;
                let size = (original_size as f64 / ratio) as u64;
                (size, "near_lossless".into(), 1.0 - max_error)
            }
            CompressionStrategy::LossyWithRoi { quality, roi_pct } => {
                // Ratio ~10-20x pour images
                let base_ratio = 15.0;
                let roi_factor = 1.0 + (1.0 - roi_pct) * 3.0;
                let ratio = base_ratio * roi_factor;
                let size = (original_size as f64 / ratio) as u64;
                (size, "lossy_roi".into(), *quality)
            }
            CompressionStrategy::Semantic { feature_count } => {
                // Extraction de features: ratio très élevé
                let ratio = (original_size as f64) / (feature_count as f64 * 32.0);
                let size = (*feature_count as u64) * 32;
                (size.min(original_size), "semantic".into(), 0.85)
            }
        };

        let ratio = if compressed_size > 0 {
            original_size as f64 / compressed_size as f64
        } else {
            original_size as f64
        };

        let elapsed = start.elapsed();

        info!(
            "Compressed: {} -> {} bytes ({:.1}x) using {}",
            original_size, compressed_size, ratio, algorithm
        );

        Ok(CompressionResult2 {
            original_size,
            compressed_size,
            ratio,
            strategy: strategy.clone(),
            algorithm,
            compression_time_ms: elapsed.as_millis() as u64,
            quality_preserved: quality,
        })
    }

    /// Statistiques globales
    pub fn statistics(&self) -> CompressionStatsSummary {
        let avg_ratio = if self.stats.total_original > 0 {
            self.stats.total_compressed as f64 / self.stats.total_original as f64
        } else {
            1.0
        };

        CompressionStatsSummary {
            total_bundles_compressed: self.stats.total_compressed,
            total_bytes_saved: self.stats.total_original.saturating_sub(self.stats.total_compressed),
            average_compression_ratio: if avg_ratio > 0.0 { 1.0 / avg_ratio } else { 1.0 },
        }
    }
}

/// Résumé des statistiques de compression
#[derive(Debug, Clone, Default)]
pub struct CompressionStatsSummary {
    pub total_bundles_compressed: u64,
    pub total_bytes_saved: u64,
    pub average_compression_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_strategy_no_compression_needed() {
        let compressor = AiCompressor::new();
        let context = BandwidthContext {
            available_kbps: 10.0,
            latency_minutes: 11.0,
            contact_duration_secs: 600,
            content_type: ContentType::Text,
            original_size_bytes: 1000,
        };

        let strategy = compressor.select_strategy(&context);
        assert_eq!(strategy, CompressionStrategy::None);
    }

    #[test]
    fn test_select_strategy_compression_needed() {
        let compressor = AiCompressor::new();
        let context = BandwidthContext {
            available_kbps: 0.5,
            latency_minutes: 11.0,
            contact_duration_secs: 120,
            content_type: ContentType::ScientificImage,
            original_size_bytes: 1024 * 1024, // 1 MB
        };

        let strategy = compressor.select_strategy(&context);
        // Devrait être LossyWithRoi car ratio nécessaire > 3
        assert!(matches!(strategy, CompressionStrategy::LossyWithRoi { .. }));
    }

    #[test]
    fn test_compress_lossless() {
        let compressor = AiCompressor::new();
        let data = vec![42u8; 10000]; // Données répétitives (bien compressibles)

        let result = compressor
            .compress(&data, &CompressionStrategy::Lossless)
            .unwrap();

        assert!(result.ratio > 1.0);
        assert_eq!(result.algorithm, "zstd");
        assert_eq!(result.quality_preserved, 1.0);
    }

    #[test]
    fn test_compress_empty_fails() {
        let compressor = AiCompressor::new();
        let result = compressor.compress(&[], &CompressionStrategy::Lossless);
        assert!(result.is_err());
    }

    #[test]
    fn test_compress_semantic() {
        let compressor = AiCompressor::new();
        let data = vec![1u8; 100_000]; // 100 KB

        let result = compressor
            .compress(&data, &CompressionStrategy::Semantic { feature_count: 256 })
            .unwrap();

        // 256 features * 32 bytes = 8192 bytes
        assert!(result.compressed_size <= 8192);
        assert!(result.ratio > 5.0);
    }
}
