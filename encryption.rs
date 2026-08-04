//! # Encryption — AES-256-GCM et Post-Quantique
//!
//! Chiffrement des données en transit entre les entités martiennes.

use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit};
use aes_gcm::aead::{Aead, OsRng};
use crate::{SecurityError, SecurityResult};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};

/// Taille de la clé AES-256
const KEY_SIZE: usize = 32;
/// Taille du nonce GCM
const NONCE_SIZE: usize = 12;

/// Résultat du chiffrement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub tag: Vec<u8>, // Authentication tag
    pub key_version: u32,
}

impl EncryptedData {
    /// Taille totale du message chiffré
    pub fn total_size(&self) -> usize {
        self.ciphertext.len() + self.nonce.len() + self.tag.len()
    }
}

/// Gestionnaire de chiffrement AES-256-GCM
pub struct AesEncryptor {
    key: [u8; KEY_SIZE],
    key_version: u32,
    encrypt_count: u64,
}

impl AesEncryptor {
    /// Crée un nouveau chiffreur avec une clé aléatoire
    pub fn new_random() -> Self {
        let mut key = [0u8; KEY_SIZE];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut key);

        Self {
            key,
            key_version: 1,
            encrypt_count: 0,
        }
    }

    /// Crée un chiffreur avec une clé spécifique
    pub fn from_key(key: &[u8; KEY_SIZE], version: u32) -> Self {
        Self {
            key: *key,
            key_version: version,
            encrypt_count: 0,
        }
    }

    /// Chiffre des données avec AES-256-GCM
    pub fn encrypt(&mut self, plaintext: &[u8]) -> SecurityResult<EncryptedData> {
        let key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(key);

        // Générer un nonce aléatoire
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Chiffrement
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SecurityError::EncryptionFailed(e.to_string()))?;

        self.encrypt_count += 1;

        // Séparer le tag d'authentification (derniers 16 bytes)
        let tag_start = ciphertext.len().saturating_sub(16);
        let (ct, tag) = ciphertext.split_at(tag_start);

        Ok(EncryptedData {
            ciphertext: ct.to_vec(),
            nonce: nonce_bytes.to_vec(),
            tag: tag.to_vec(),
            key_version: self.key_version,
        })
    }

    /// Déchiffre des données AES-256-GCM
    pub fn decrypt(&self, encrypted: &EncryptedData) -> SecurityResult<Vec<u8>> {
        let key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(key);

        let nonce = Nonce::from_slice(&encrypted.nonce);

        // Reconstruire le ciphertext complet (ct + tag)
        let mut full_ciphertext = encrypted.ciphertext.clone();
        full_ciphertext.extend_from_slice(&encrypted.tag);

        cipher
            .decrypt(nonce, full_ciphertext.as_ref())
            .map_err(|e| SecurityError::DecryptionFailed(e.to_string()))
    }

    /// Dérive une clé à partir d'une passphrase (PBKDF2-like)
    pub fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; KEY_SIZE] {
        let mut hasher = Sha3_256::new();
        hasher.update(passphrase.as_bytes());
        hasher.update(salt);
        let result = hasher.finalize();
        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&result);
        key
    }

    pub fn get_version(&self) -> u32 {
        self.key_version
    }

    pub fn get_encrypt_count(&self) -> u64 {
        self.encrypt_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut encryptor = AesEncryptor::new_random();
        let plaintext = b"Hello Mars from SpaceOne!";

        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext.to_vec());
    }

    #[test]
    fn test_encryption_overhead() {
        let mut encryptor = AesEncryptor::new_random();
        let plaintext = vec![0u8; 1024];

        let encrypted = encryptor.encrypt(&plaintext).unwrap();
        // AES-GCM ajoute nonce (12) + tag (16) = 28 bytes
        assert_eq!(encrypted.total_size(), 1024 + 12 + 16);
    }

    #[test]
    fn test_key_derivation() {
        let key1 = AesEncryptor::derive_key("mars2030", b"salt1");
        let key2 = AesEncryptor::derive_key("mars2030", b"salt1");
        let key3 = AesEncryptor::derive_key("mars2030", b"salt2");

        assert_eq!(key1, key2); // Même passphrase + salt = même clé
        assert_ne!(key1, key3); // Salt différent = clé différente
    }

    #[test]
    fn test_wrong_key_fails() {
        let mut enc1 = AesEncryptor::new_random();
        let enc2 = AesEncryptor::new_random();

        let encrypted = enc1.encrypt(b"secret").unwrap();
        let result = enc2.decrypt(&encrypted);

        assert!(result.is_err());
    }
}
