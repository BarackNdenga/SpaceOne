//! # Signing — Signature des Bundles RAUC
//!
//! Signature cryptographique des images firmware et des bundles
//! de communication pour garantir l'intégrité et l'authenticité.

use crate::{SecurityError, SecurityResult};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Algorithme de signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    Ed25519,
    ECDSAP256,
    PostQuantumMLDSA, // ML-DSA (Dilithium) pour post-quantique
}

/// Clé de signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKey {
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub owner: String,
}

impl SigningKey {
    /// Génère une nouvelle paire de clés (simulée)
    pub fn generate(algorithm: SignatureAlgorithm, owner: String) -> Self {
        use rand::RngCore;
        let mut private_key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key);

        let mut hasher = Sha3_256::new();
        hasher.update(&private_key);
        let public_key = hasher.finalize().to_vec();

        Self {
            key_id: format!("key-{}", &hex::encode(&public_key[..8])),
            algorithm,
            public_key,
            private_key,
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(365),
            owner,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Signature d'un bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSignature {
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub signature: Vec<u8>,
    pub signed_at: DateTime<Utc>,
    pub bundle_hash: String,
    pub expires_at: DateTime<Utc>,
}

/// Signataire de bundles RAUC
pub struct RaucSigner {
    keys: HashMap<String, SigningKey>,
    default_key_id: Option<String>,
}

impl RaucSigner {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            default_key_id: None,
        }
    }

    /// Ajoute une clé de signature
    pub fn add_key(&mut self, key: SigningKey) {
        if self.default_key_id.is_none() {
            self.default_key_id = Some(key.key_id.clone());
        }
        self.keys.insert(key.key_id.clone(), key);
    }

    /// Signe un bundle de données
    pub fn sign_bundle(&self, data: &[u8], key_id: Option<&str>) -> SecurityResult<BundleSignature> {
        let kid = key_id.or_else(|| self.default_key_id.as_deref())
            .ok_or_else(|| SecurityError::InvalidKey("No key available".into()))?;

        let key = self.keys.get(kid)
            .ok_or_else(|| SecurityError::InvalidKey(format!("Key not found: {}", kid)))?;

        if key.is_expired() {
            return Err(SecurityError::InvalidKey("Key expired".into()));
        }

        // Calculer le hash du bundle
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        let hash_hex = hex::encode(&hash);

        // Simuler la signature (hash du hash + clé privée)
        let mut sig_hasher = Sha3_256::new();
        sig_hasher.update(&hash);
        sig_hasher.update(&key.private_key);
        let signature = sig_hasher.finalize().to_vec();

        Ok(BundleSignature {
            key_id: kid.to_string(),
            algorithm: key.algorithm.clone(),
            signature,
            signed_at: Utc::now(),
            bundle_hash: hash_hex,
            expires_at: key.expires_at,
        })
    }

    /// Vérifie une signature de bundle
    pub fn verify_bundle(&self, data: &[u8], signature: &BundleSignature) -> SecurityResult<bool> {
        let key = self.keys.get(&signature.key_id)
            .ok_or_else(|| SecurityError::InvalidKey("Signer key not found".into()))?;

        // Vérifier l'expiration
        if signature.expires_at < Utc::now() {
            return Err(SecurityError::InvalidKey("Signature expired".into()));
        }

        // Recalculer le hash
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let hash_hex = hex::encode(hasher.finalize());

        // Vérifier le hash correspond
        if hash_hex != signature.bundle_hash {
            return Err(SecurityError::IntegrityViolation(
                "Bundle hash mismatch".into(),
            ));
        }

        // Vérifier la signature
        let mut sig_hasher = Sha3_256::new();
        sig_hasher.update(&hex::decode(&signature.bundle_hash).unwrap_or_default());
        sig_hasher.update(&key.private_key);
        let expected_sig = sig_hasher.finalize().to_vec();

        Ok(expected_sig == signature.signature)
    }
}

/// Vérificateur d'intégrité de bundle RAUC
pub struct RaucVerifier {
    trusted_keys: HashMap<String, Vec<u8>>, // key_id -> public_key
}

impl RaucVerifier {
    pub fn new() -> Self {
        Self {
            trusted_keys: HashMap::new(),
        }
    }

    pub fn add_trusted_key(&mut self, key_id: String, public_key: Vec<u8>) {
        self.trusted_keys.insert(key_id, public_key);
    }

    pub fn verify(&self, data: &[u8], signature: &BundleSignature) -> bool {
        // Vérifier que la clé est de confiance
        let trusted = self.trusted_keys.contains_key(&signature.key_id);
        if !trusted {
            return false;
        }

        // Vérifier le hash
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let hash_hex = hex::encode(hasher.finalize());

        hash_hex == signature.bundle_hash && signature.expires_at > Utc::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let mut signer = RaucSigner::new();
        let key = SigningKey::generate(SignatureAlgorithm::Ed25519, "spaceone".into());
        signer.add_key(key);

        let data = b"RAUC bundle content for Mars rover";
        let signature = signer.sign_bundle(data, None).unwrap();

        assert!(signer.verify_bundle(data, &signature).unwrap());
    }

    #[test]
    fn test_tampered_data_fails() {
        let mut signer = RaucSigner::new();
        let key = SigningKey::generate(SignatureAlgorithm::Ed25519, "spaceone".into());
        signer.add_key(key);

        let data = b"original data";
        let signature = signer.sign_bundle(data, None).unwrap();

        let tampered = b"tampered data";
        assert!(signer.verify_bundle(tampered, &signature).is_err());
    }

    #[test]
    fn test_rauc_verifier() {
        let mut signer = RaucSigner::new();
        let key = SigningKey::generate(SignatureAlgorithm::Ed25519, "nasa".into());
        let pub_key = key.public_key.clone();
        let kid = key.key_id.clone();
        signer.add_key(key);

        let mut verifier = RaucVerifier::new();
        verifier.add_trusted_key(kid.clone(), pub_key);

        let data = b"verified bundle";
        let sig = signer.sign_bundle(data, None).unwrap();
        assert!(verifier.verify(data, &sig));
    }
}
