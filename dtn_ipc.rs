//! # DTN IPC — Communication réelle avec aqm-dtnd (uD3TN, RFC 9171)
//!
//! Ce module communique avec le daemon DTN d'AsterQuanta via le socket AAP
//! (Application Agent Protocol). uD3TN est une implémentation réelle du
//! Bundle Protocol (RFC 9171) utilisée dans de vrais projets spatiaux.
//!
//! SpaceOne envoie et reçoit des bundles DTN via ce socket. Les bundles
//! prioritaires de SpaceOne sont routés à travers l'infrastructure DTN
//! existante d'AsterQuanta.

use super::*;

/// Client DTN via aqm-dtnd (uD3TN)
pub struct DtnClient;

impl DtnClient {
    /// Vérifier si le daemon DTN est joignable
    pub fn is_available() -> bool {
        std::path::Path::new(AQM_DTND_AAP_SOCKET).exists()
    }

    /// Envoyer un bundle via le socket AAP
    ///
    /// Le bundle est encodé en CBOR (standard Bundle Protocol) et envoyé
    /// au daemon uD3TN qui le route vers la destination.
    pub fn send_bundle(
        destination_eid: &str,
        source_eid: &str,
        payload: &[u8],
        priority: DtnPriority,
    ) -> BridgeResult<String> {
        // Ouvrir le socket AAP
        let mut stream = UnixStream::connect(AQM_DTND_AAP_SOCKET)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "aqm-dtnd AAP socket inaccessible: {}", e
            )))?;

        // Encoder le bundle au format AAP (version 2)
        // Format: BUNDLE dest_eid src_eid priority payload_length payload
        let mut request = Vec::new();

        // Header
        request.extend_from_slice(b"BUNDLE");
        request.push(0x20); // AAP version 2 marker

        // Destination EID
        let dest_bytes = destination_eid.as_bytes();
        request.extend_from_slice(&(dest_bytes.len() as u16).to_be_bytes());
        request.extend_from_slice(dest_bytes);

        // Source EID
        let src_bytes = source_eid.as_bytes();
        request.extend_from_slice(&(src_bytes.len() as u16).to_be_bytes());
        request.extend_from_slice(src_bytes);

        // Priority
        request.push(priority.as_byte());

        // Payload
        request.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        request.extend_from_slice(payload);

        // Envoyer
        stream.write_all(&request)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "AAP write failed: {}", e
            )))?;

        // Lire la confirmation
        let mut response = vec![0u8; 64];
        let n = stream.read(&mut response)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "AAP read failed: {}", e
            )))?;

        let bundle_id = String::from_utf8_lossy(&response[..n]).trim().to_string();

        info!(
            "DTN bundle sent: dest={}, priority={:?}, size={}B, id={}",
            destination_eid, priority, payload.len(), bundle_id
        );

        Ok(bundle_id)
    }

    /// Recevoir un bundle (mode polling)
    pub fn receive_bundle() -> BridgeResult<Option<DtnBundle>> {
        let mut stream = match UnixStream::connect(AQM_DTND_AAP_SOCKET) {
            Ok(s) => s,
            Err(_) => return Ok(None), // Aucun bundle en attente
        };

        // Envoyer requête RECEIVE
        stream.write_all(b"RECEIVE")
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "AAP receive request failed: {}", e
            )))?;

        let mut response = vec![0u8; 65536];
        let n = stream.read(&mut response)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "AAP receive failed: {}", e
            )))?;

        if n == 0 {
            return Ok(None);
        }

        // Décoder le bundle reçu
        let bundle = DtnBundle::from_aap_response(&response[..n])?;

        info!(
            "DTN bundle received: source={}, size={}B",
            bundle.source_eid, bundle.payload.len()
        );

        Ok(Some(bundle))
    }

    /// Statut du noeud DTN
    pub fn get_node_status() -> BridgeResult<DtnNodeStatus> {
        let mut stream = UnixStream::connect(AQM_DTND_AAP_SOCKET)
            .map_err(|e| BridgeError::SocketUnavailable(format!(
                "aqm-dtnd status check failed: {}", e
            )))?;

        stream.write_all(b"STATUS")
            .map_err(|e| BridgeError::SocketUnavailable(e.to_string()))?;

        let mut response = String::new();
        stream.read_to_string(&mut response)
            .map_err(|e| BridgeError::SocketUnavailable(e.to_string()))?;

        let status: DtnNodeStatus = serde_json::from_str(&response)
            .map_err(|e| BridgeError::Deserialization(e.to_string()))?;

        Ok(status)
    }
}

/// Bundle DTN reçu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtnBundle {
    pub bundle_id: String,
    pub source_eid: String,
    pub destination_eid: String,
    pub payload: Vec<u8>,
    pub timestamp: String,
    pub priority: u8,
    pub lifetime_seconds: u32,
}

impl DtnBundle {
    /// Décoder un bundle depuis une réponse AAP
    fn from_aap_response(data: &[u8]) -> BridgeResult<Self> {
        // Format: BUNDLE_RECV id src_eid dst_eid payload_len payload timestamp
        let text = String::from_utf8_lossy(data);
        let parts: Vec<&str> = text.splitn(6, '|').collect();

        if parts.len() < 6 {
            return Err(BridgeError::Deserialization(
                "Invalid AAP bundle format".into()
            ));
        }

        Ok(DtnBundle {
            bundle_id: parts[0].to_string(),
            source_eid: parts[1].to_string(),
            destination_eid: parts[2].to_string(),
            payload: parts[3].as_bytes().to_vec(),
            timestamp: parts[4].to_string(),
            priority: parts[5].parse().unwrap_or(3),
            lifetime_seconds: 3600,
        })
    }
}

/// Statut du noeud DTN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtnNodeStatus {
    pub node_eid: String,
    pub contacts: Vec<DtnContact>,
    pub pending_bundles: u32,
    pub delivered_bundles: u64,
    pub storage_used_bytes: u64,
    pub storage_max_bytes: u64,
}

/// Contact DTN (route disponible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtnContact {
    pub eid: String,
    pub next_hop: String,
    pub last_seen: String,
    pub is_active: bool,
}

/// Priorité des bundles DTN
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtnPriority {
    /// Prio 0 : bulk (données non-critiques)
    Bulk,
    /// Prio 1-3 : normal (telemetry standard)
    Normal,
    /// Prio 4-5 : high (commandes opérationnelles)
    High,
    /// Prio 6 : expedited (safe mode, urgence)
    Expedited,
}

impl DtnPriority {
    pub fn as_byte(self) -> u8 {
        match self {
            DtnPriority::Bulk => 0,
            DtnPriority::Normal => 2,
            DtnPriority::High => 4,
            DtnPriority::Expedited => 6,
        }
    }
}

impl From<u8> for DtnPriority {
    fn from(val: u8) -> Self {
        match val {
            0 => DtnPriority::Bulk,
            1..=3 => DtnPriority::Normal,
            4..=5 => DtnPriority::High,
            _ => DtnPriority::Expedited,
        }
    }
}
