//! Ed25519 signing for app-managed keypairs.
//!
//! Every payload struct (`TimingEvent`, `EventInfo`, `ResultSnapshot`) carries
//! optional `signing_key` + `signature` fields.  The device signs at source;
//! transport never touches the signature.
//!
//! [`DeviceKeys`] — local device keypair (generate, sign, verify, cache).
//! [`SigningKeyRegistry`] — TOFU trust store for all seen keys.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Current timestamp in ms since epoch.
fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

// ---------------------------------------------------------------------------
// Canonical payload — the bytes that get signed
// ---------------------------------------------------------------------------

/// Build the canonical JSON payload for signing.
///
/// Serialises `payload` to JSON, strips the `signature` and `signing_key`
/// fields, then re-serialises to a deterministic string.  Rust's
/// `serde_json::Map` preserves field insertion order, so this is stable per
/// struct layout.
pub fn canonical_payload<T: Serialize>(payload: &T) -> Result<String, SigningError> {
    let mut val = serde_json::to_value(payload)
        .map_err(|e| SigningError::SerializationError(e.to_string()))?;
    if let Some(obj) = val.as_object_mut() {
        obj.remove("signature");
        obj.remove("signing_key");
    }
    serde_json::to_string(&val).map_err(|e| SigningError::SerializationError(e.to_string()))
}

/// Sign a payload struct: compute canonical JSON, sign with device key.
///
/// Returns `(signature_b64, signing_key_b64)`.
pub fn sign_payload<T: Serialize>(
    payload: &T,
    device_keys: &DeviceKeys,
) -> Result<(String, String), SigningError> {
    let canonical = canonical_payload(payload)?;
    let signature = device_keys.sign(canonical.as_bytes())?;
    Ok((signature, device_keys.ed25519_public_key.clone()))
}

/// Verify a payload struct: recompute canonical JSON, check signature.
pub fn verify_payload<T: Serialize>(
    payload: &T,
    signature_b64: &str,
    signing_key_b64: &str,
) -> Result<(), SigningError> {
    let canonical = canonical_payload(payload)?;
    DeviceKeys::verify(canonical.as_bytes(), signature_b64, signing_key_b64)
}

// ---------------------------------------------------------------------------
// Device keys — the local device's signing keypair
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const DEVICE_KEY_STORAGE: &str = "kt_device_keys";

/// Local device signing keypair.
///
/// Generated on first use, persisted in localStorage.  The private key never
/// leaves the device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceKeys {
    pub user_id: String,
    pub device_id: String,
    /// Base64-encoded Ed25519 public key (32 bytes).
    pub ed25519_public_key: String,
    /// Base64-encoded Ed25519 private key (32 bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ed25519_private_key: Option<String>,
}

impl DeviceKeys {
    /// Generate a new keypair for this device.
    pub fn generate(user_id: String, device_id: String) -> Self {
        let mut csprng = rand_core::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        Self {
            user_id,
            device_id,
            ed25519_public_key: BASE64.encode(verifying_key.as_bytes()),
            ed25519_private_key: Some(BASE64.encode(signing_key.to_bytes())),
        }
    }

    /// Wrap a public key only (cannot sign — used for verification).
    pub fn from_public_key(
        user_id: String,
        device_id: String,
        ed25519_public_key_b64: String,
    ) -> Self {
        Self {
            user_id,
            device_id,
            ed25519_public_key: ed25519_public_key_b64,
            ed25519_private_key: None,
        }
    }

    /// Sign bytes with the private key.  Returns base64 signature.
    pub fn sign(&self, payload: &[u8]) -> Result<String, SigningError> {
        let private_key_b64 = self
            .ed25519_private_key
            .as_ref()
            .ok_or(SigningError::NoPrivateKey)?;

        let key_bytes = BASE64
            .decode(private_key_b64)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        let bytes: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| SigningError::InvalidKey("private key must be 32 bytes".into()))?;

        let signing_key = SigningKey::from_bytes(&bytes);
        let signature = signing_key.sign(payload);
        Ok(BASE64.encode(signature.to_bytes()))
    }

    /// Verify an Ed25519 signature against a public key (base64).
    pub fn verify(
        payload: &[u8],
        signature_b64: &str,
        public_key_b64: &str,
    ) -> Result<(), SigningError> {
        let sig_bytes = BASE64
            .decode(signature_b64)
            .map_err(|e| SigningError::InvalidSignature(e.to_string()))?;
        let signature_bytes: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| SigningError::InvalidSignature("signature must be 64 bytes".into()))?;

        let key_bytes = BASE64
            .decode(public_key_b64)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| SigningError::InvalidKey("public key must be 32 bytes".into()))?;

        let verifying_key = VerifyingKey::from_bytes(&key_array)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);

        verifying_key
            .verify(payload, &signature)
            .map_err(|e| SigningError::VerificationFailed(e.to_string()))
    }

    /// 8-char hex fingerprint for visual verification.
    pub fn fingerprint(&self) -> Result<String, SigningError> {
        let key_bytes = BASE64
            .decode(&self.ed25519_public_key)
            .map_err(|e| SigningError::InvalidKey(e.to_string()))?;
        Ok(key_bytes[..4].iter().map(|b| format!("{b:02x}")).collect())
    }

    // -----------------------------------------------------------------------
    // localStorage persistence
    // -----------------------------------------------------------------------

    #[cfg(target_arch = "wasm32")]
    pub fn save_to_storage(&self) -> Result<(), SigningError> {
        let window = web_sys::window().ok_or(SigningError::NoWindow)?;
        let storage = window
            .local_storage()
            .map_err(|e| SigningError::StorageError(format!("{e:?}")))?
            .ok_or(SigningError::NoStorage)?;
        let json = serde_json::to_string(self)
            .map_err(|e| SigningError::SerializationError(e.to_string()))?;
        storage
            .set_item(DEVICE_KEY_STORAGE, &json)
            .map_err(|e| SigningError::StorageError(format!("{e:?}")))?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_from_storage() -> Option<Self> {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        let json = storage.get_item(DEVICE_KEY_STORAGE).ok()??;
        serde_json::from_str(&json).ok()
    }

    /// Load or generate on first use.
    #[cfg(target_arch = "wasm32")]
    pub fn load_or_generate(user_id: &str, device_id: &str) -> Self {
        Self::load_from_storage().unwrap_or_else(|| {
            let keys = Self::generate(user_id.to_owned(), device_id.to_owned());
            let _ = keys.save_to_storage();
            keys
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_storage(&self) -> Result<(), SigningError> {
        Err(SigningError::StorageError(
            "localStorage not available outside WASM".into(),
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_storage() -> Option<Self> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_or_generate(user_id: &str, device_id: &str) -> Self {
        Self::generate(user_id.to_owned(), device_id.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Signing key registry — TOFU trust store
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const REGISTRY_STORAGE: &str = "kt_signing_keys";

/// Trust status for a signing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyTrustStatus {
    /// User explicitly confirmed this key.
    Verified,
    /// Seen but not confirmed (default for first encounter).
    Unverified,
    /// User marked as untrusted.
    Rejected,
}

/// A recorded signing key — one entry per distinct device key seen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKeyRecord {
    /// Base64 Ed25519 public key (primary key for lookups).
    pub public_key: String,
    /// Matrix user ID of the owner (from `official_id` or `sender`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Linked Contact.user_id (manual association).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
    /// Human-readable label ("Alice's phone", "Event laptop").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub status: KeyTrustStatus,
}

/// In-memory + localStorage registry of all seen signing keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SigningKeyRegistry {
    keys: Vec<SigningKeyRecord>,
}

impl SigningKeyRegistry {
    /// Load from localStorage (or empty).
    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return Self::default(),
        };
        let storage = match window.local_storage() {
            Ok(Some(s)) => s,
            _ => return Self::default(),
        };
        let json = match storage.get_item(REGISTRY_STORAGE) {
            Ok(Some(j)) => j,
            _ => return Self::default(),
        };
        serde_json::from_str(&json).unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        Self::default()
    }

    /// Persist to localStorage.
    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) -> Result<(), SigningError> {
        let window = web_sys::window().ok_or(SigningError::NoWindow)?;
        let storage = window
            .local_storage()
            .map_err(|e| SigningError::StorageError(format!("{e:?}")))?
            .ok_or(SigningError::NoStorage)?;
        let json = serde_json::to_string(self)
            .map_err(|e| SigningError::SerializationError(e.to_string()))?;
        storage
            .set_item(REGISTRY_STORAGE, &json)
            .map_err(|e| SigningError::StorageError(format!("{e:?}")))?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) -> Result<(), SigningError> {
        Ok(())
    }

    /// Record a key sighting.  Returns the record (new or updated).
    pub fn record_key(&mut self, public_key: &str, user_id: Option<&str>) -> &SigningKeyRecord {
        let now = now_ms();
        if let Some(rec) = self.keys.iter_mut().find(|k| k.public_key == public_key) {
            rec.last_seen = now;
            if let Some(uid) = user_id {
                if rec.user_id.is_none() {
                    rec.user_id = Some(uid.to_owned());
                }
            }
        } else {
            self.keys.push(SigningKeyRecord {
                public_key: public_key.to_owned(),
                user_id: user_id.map(|s| s.to_owned()),
                contact_id: None,
                label: None,
                first_seen: now,
                last_seen: now,
                status: KeyTrustStatus::Unverified,
            });
        }
        self.keys
            .iter()
            .find(|k| k.public_key == public_key)
            .unwrap()
    }

    /// Look up a key by public key.
    pub fn find_key(&self, public_key: &str) -> Option<&SigningKeyRecord> {
        self.keys.iter().find(|k| k.public_key == public_key)
    }

    /// Look up all keys for a contact.
    pub fn find_by_contact(&self, contact_id: &str) -> Vec<&SigningKeyRecord> {
        self.keys
            .iter()
            .filter(|k| k.contact_id.as_deref() == Some(contact_id))
            .collect()
    }

    /// Link a key to a contact.
    pub fn link_to_contact(&mut self, public_key: &str, contact_id: &str) {
        if let Some(rec) = self.keys.iter_mut().find(|k| k.public_key == public_key) {
            rec.contact_id = Some(contact_id.to_owned());
        }
    }

    /// Set trust status.
    pub fn set_status(&mut self, public_key: &str, status: KeyTrustStatus) {
        if let Some(rec) = self.keys.iter_mut().find(|k| k.public_key == public_key) {
            rec.status = status;
        }
    }

    /// All records.
    pub fn all(&self) -> &[SigningKeyRecord] {
        &self.keys
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SigningError {
    NoPrivateKey,
    InvalidKey(String),
    InvalidSignature(String),
    VerificationFailed(String),
    NoWindow,
    NoStorage,
    StorageError(String),
    SerializationError(String),
}

impl std::fmt::Display for SigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPrivateKey => write!(f, "no private key available for signing"),
            Self::InvalidKey(e) => write!(f, "invalid key: {e}"),
            Self::InvalidSignature(e) => write!(f, "invalid signature: {e}"),
            Self::VerificationFailed(e) => write!(f, "signature verification failed: {e}"),
            Self::NoWindow => write!(f, "no window object available"),
            Self::NoStorage => write!(f, "no localStorage available"),
            Self::StorageError(e) => write!(f, "storage error: {e}"),
            Self::SerializationError(e) => write!(f, "serialization error: {e}"),
        }
    }
}

impl std::error::Error for SigningError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[test]
    fn sign_and_verify_roundtrip() {
        let keys = DeviceKeys::generate("alice".into(), "DEVICE1".into());
        let payload = b"hello khanatime";
        let sig = keys.sign(payload).unwrap();
        DeviceKeys::verify(payload, &sig, &keys.ed25519_public_key).unwrap();
    }

    #[test]
    fn verify_fails_with_wrong_key() {
        let keys1 = DeviceKeys::generate("alice".into(), "D1".into());
        let keys2 = DeviceKeys::generate("bob".into(), "D2".into());
        let payload = b"important data";
        let sig = keys1.sign(payload).unwrap();
        assert!(DeviceKeys::verify(payload, &sig, &keys2.ed25519_public_key).is_err());
    }

    #[test]
    fn verify_fails_with_tampered_payload() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        let payload = b"original data";
        let sig = keys.sign(payload).unwrap();
        assert!(DeviceKeys::verify(b"tampered data", &sig, &keys.ed25519_public_key).is_err());
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        assert_eq!(keys.fingerprint().unwrap(), keys.fingerprint().unwrap());
    }

    #[test]
    fn fingerprint_is_8_hex_chars() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        let fp = keys.fingerprint().unwrap();
        assert_eq!(fp.len(), 8);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn from_public_key_cannot_sign() {
        let keys =
            DeviceKeys::from_public_key("alice".into(), "D1".into(), BASE64.encode([42u8; 32]));
        assert!(keys.sign(b"test").is_err());
    }

    #[test]
    fn serialization_roundtrip() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        let json = serde_json::to_string(&keys).unwrap();
        let restored: DeviceKeys = serde_json::from_str(&json).unwrap();
        assert_eq!(keys, restored);
    }

    #[test]
    fn serialization_skips_private_key_when_none() {
        let keys =
            DeviceKeys::from_public_key("alice".into(), "D1".into(), BASE64.encode([1u8; 32]));
        let json = serde_json::to_string(&keys).unwrap();
        assert!(!json.contains("ed25519_private_key"));
    }

    // -- canonical_payload tests --

    #[derive(Serialize)]
    struct TestPayload {
        name: String,
        value: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        signing_key: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    }

    #[test]
    fn canonical_payload_strips_signing_fields() {
        let payload = TestPayload {
            name: "test".into(),
            value: 42,
            signing_key: Some("k".into()),
            signature: Some("s".into()),
        };
        let canonical = canonical_payload(&payload).unwrap();
        assert!(!canonical.contains("signing_key"));
        assert!(!canonical.contains("signature"));
        assert!(canonical.contains("\"name\":\"test\""));
        assert!(canonical.contains("\"value\":42"));
    }

    #[test]
    fn canonical_payload_deterministic() {
        let p1 = TestPayload {
            name: "x".into(),
            value: 1,
            signing_key: None,
            signature: None,
        };
        let p2 = TestPayload {
            name: "x".into(),
            value: 1,
            signing_key: Some("a".into()),
            signature: Some("b".into()),
        };
        assert_eq!(
            canonical_payload(&p1).unwrap(),
            canonical_payload(&p2).unwrap()
        );
    }

    #[test]
    fn sign_and_verify_payload_roundtrip() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        let payload = TestPayload {
            name: "car 7".into(),
            value: 500,
            signing_key: None,
            signature: None,
        };
        let (sig, key) = sign_payload(&payload, &keys).unwrap();
        verify_payload(&payload, &sig, &key).unwrap();
    }

    #[test]
    fn verify_payload_fails_with_tampered_data() {
        let keys = DeviceKeys::generate("alice".into(), "D1".into());
        let payload = TestPayload {
            name: "car 7".into(),
            value: 500,
            signing_key: None,
            signature: None,
        };
        let (sig, key) = sign_payload(&payload, &keys).unwrap();
        let tampered = TestPayload {
            name: "car 7".into(),
            value: 999,
            signing_key: None,
            signature: None,
        };
        assert!(verify_payload(&tampered, &sig, &key).is_err());
    }

    // -- SigningKeyRegistry tests --

    #[test]
    fn registry_record_and_find() {
        let mut reg = SigningKeyRegistry::default();
        reg.record_key("key1", Some("@alice:server"));
        let rec = reg.find_key("key1").unwrap();
        assert_eq!(rec.user_id.as_deref(), Some("@alice:server"));
        assert_eq!(rec.status, KeyTrustStatus::Unverified);
    }

    #[test]
    fn registry_link_to_contact() {
        let mut reg = SigningKeyRegistry::default();
        reg.record_key("key1", None);
        reg.link_to_contact("key1", "alice");
        let linked = reg.find_by_contact("alice");
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].public_key, "key1");
    }

    #[test]
    fn registry_set_status() {
        let mut reg = SigningKeyRegistry::default();
        reg.record_key("key1", None);
        reg.set_status("key1", KeyTrustStatus::Verified);
        assert_eq!(
            reg.find_key("key1").unwrap().status,
            KeyTrustStatus::Verified
        );
    }
}
