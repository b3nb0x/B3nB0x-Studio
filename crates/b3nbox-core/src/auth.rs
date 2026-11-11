use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub name: String,
    pub role: String,
    /// Hash des Keys - niemals der Klartext-Key.
    /// Format: "s512$<salt-hex>$<hash-hex>" (gesalzenes SHA-512) oder
    /// Legacy: 64 Hex-Zeichen (ungesalzenes SHA-256, wird noch akzeptiert).
    pub key_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ttl")]
    pub session_ttl_secs: u64,
    #[serde(default)]
    pub keys: Vec<ApiKeyEntry>,
}

fn default_enabled() -> bool {
    true
}

fn default_ttl() -> u64 {
    3600 * 12
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthInfo {
    pub name: String,
    pub role: String,
}

impl AuthInfo {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// Erzeugt den SHA-256-Hash eines API-Keys (Hex) - Legacy-Format.
pub fn hash_key(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    format!("{digest:x}")
}

/// Prueft einen API-Key gegen einen gespeicherten Hash.
/// Unterstuetzt das aktuelle Format ("s512$<salt>$<hash>", gesalzenes
/// SHA-512) und das Legacy-Format (ungesalzenes SHA-256, 64 Hex-Zeichen).
pub fn verify_key_hash(api_key: &str, stored: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("s512$") {
        let Some((salt_hex, hash_hex)) = rest.split_once('$') else {
            return false;
        };
        let Ok(salt) = hex_decode(salt_hex) else {
            return false;
        };
        let mut hasher = Sha512::new();
        hasher.update(&salt);
        hasher.update(api_key.as_bytes());
        let digest = hasher.finalize();
        constant_time_eq(&digest, &hex_decode(hash_hex).unwrap_or_default())
    } else {
        // Legacy: ungesalzenes SHA-256
        stored.len() == 64
            && stored.chars().all(|c| c.is_ascii_hexdigit())
            && constant_time_eq(
                &Sha256::digest(api_key.as_bytes()),
                &hex_decode(stored).unwrap_or_default(),
            )
    }
}

/// Erzeugt einen neuen gesalzenen SHA-512-Hash fuer einen API-Key.
pub fn hash_key_salted(key: &str) -> String {
    let salt_raw = uuid::Uuid::new_v4();
    let salt = salt_raw.as_bytes();
    let mut hasher = Sha512::new();
    hasher.update(salt);
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    format!("s512${}${}", hex_encode(salt), hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if !s.len().is_multiple_of(2) || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(());
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| ()))
        .collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

struct Session {
    info: AuthInfo,
    expires_at: Instant,
}

pub struct Auth {
    enabled: bool,
    keys: Vec<(String, AuthInfo)>,
    session_ttl: Duration,
    sessions: RwLock<HashMap<String, Session>>,
}

impl Auth {
    pub fn new(config: &AuthConfig) -> Result<Self, String> {
        let mut keys = Vec::new();
        for entry in &config.keys {
            let legacy = entry.key_hash.len() == 64
                && entry.key_hash.chars().all(|c| c.is_ascii_hexdigit());
            let salted = entry.key_hash.starts_with("s512$")
                && entry.key_hash.split('$').count() == 3;
            if !legacy && !salted {
                return Err(format!(
                    "Key '{}' hat einen ungueltigen Hash (erwartet: 's512$<salt>$<hash>' oder 64 Hex-Zeichen). Nutze --generate-key.",
                    entry.name
                ));
            }
            if legacy {
                tracing::warn!(
                    "Key '{}' nutzt noch den ungesalzenen SHA-256-Legacy-Hash - neu generieren (--generate-key) und key_hash ersetzen",
                    entry.name
                );
            }
            keys.push((
                entry.key_hash.clone(),
                AuthInfo {
                    name: entry.name.clone(),
                    role: entry.role.clone(),
                },
            ));
        }
        if config.enabled && keys.is_empty() {
            return Err("Auth aktiviert, aber keine Keys konfiguriert".into());
        }
        Ok(Self {
            enabled: config.enabled,
            keys,
            session_ttl: Duration::from_secs(config.session_ttl_secs),
            sessions: RwLock::new(HashMap::new()),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Prueft einen API-Key und legt eine Session an. Gibt den
    /// Session-Token zurueck.
    pub fn login(&self, api_key: &str) -> Option<String> {
        let info = self.verify_key(api_key)?;
        let token = format!("bxsess-{}", uuid::Uuid::new_v4().simple());
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, s| s.expires_at > Instant::now());
        sessions.insert(
            token.clone(),
            Session {
                info,
                expires_at: Instant::now() + self.session_ttl,
            },
        );
        Some(token)
    }

    /// Prueft einen Bearer-Token (API-Key direkt oder Session-Token).
    pub fn verify(&self, token: &str) -> Option<AuthInfo> {
        if let Some(session) = self.sessions.read().unwrap().get(token) {
            if session.expires_at > Instant::now() {
                return Some(session.info.clone());
            }
            return None;
        }
        self.verify_key(token)
    }

    fn verify_key(&self, api_key: &str) -> Option<AuthInfo> {
        self.keys
            .iter()
            .find(|(key_hash, _)| verify_key_hash(api_key, key_hash))
            .map(|(_, info)| info.clone())
    }

    pub fn logout(&self, token: &str) -> bool {
        self.sessions.write().unwrap().remove(token).is_some()
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .read()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.expires_at > Instant::now())
            .count()
    }
}

/// Generiert einen neuen API-Key mit zugehoerigem (gesalzenem) Hash fuer die Config.
pub fn generate_key() -> (String, String) {
    let key = format!("bxk-{}", uuid::Uuid::new_v4());
    let hash = hash_key_salted(&key);
    (key, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_key(role: &str) -> AuthConfig {
        let (_, hash) = test_key();
        AuthConfig {
            enabled: true,
            session_ttl_secs: 3600,
            keys: vec![ApiKeyEntry {
                name: "test".into(),
                role: role.into(),
                key_hash: hash,
            }],
        }
    }

    fn test_key() -> (String, String) {
        let key = "bxk-test-key-123".to_string();
        let hash = hash_key(&key);
        (key, hash)
    }

    #[test]
    fn hash_is_sha256_hex() {
        let hash = hash_key("hallo");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash, hash_key("hallo"));
        assert_ne!(hash, hash_key("andere"));
    }

    #[test]
    fn salted_hash_roundtrip() {
        let hash = hash_key_salted("bxk-mein-key");
        assert!(hash.starts_with("s512$"));
        assert!(verify_key_hash("bxk-mein-key", &hash));
        assert!(!verify_key_hash("bxk-falsch", &hash));
        // Gleicher Key, neuer Hash (neues Salt) - beide gelten
        let hash2 = hash_key_salted("bxk-mein-key");
        assert_ne!(hash, hash2);
        assert!(verify_key_hash("bxk-mein-key", &hash2));
    }

    #[test]
    fn legacy_sha256_hash_still_accepted() {
        let hash = hash_key("bxk-legacy-key");
        assert!(verify_key_hash("bxk-legacy-key", &hash));
        assert!(!verify_key_hash("bxk-andere", &hash));
    }

    #[test]
    fn login_and_verify_roundtrip() {
        let (key, _) = test_key();
        let auth = Auth::new(&config_with_key("admin")).unwrap();

        let token = auth.login(&key).expect("Login muss funktionieren");
        assert!(token.starts_with("bxsess-"));
        let info = auth.verify(&token).expect("Session-Token muss gelten");
        assert_eq!(info.role, "admin");
        assert!(info.is_admin());

        // API-Key direkt geht auch (ohne Login)
        assert_eq!(auth.verify(&key).unwrap().name, "test");

        // Falscher Key abgelehnt
        assert!(auth.verify("bxk-falsch").is_none());
        assert!(auth.login("bxk-falsch").is_none());
    }

    #[test]
    fn logout_invalidates_session() {
        let (key, _) = test_key();
        let auth = Auth::new(&config_with_key("user")).unwrap();
        let token = auth.login(&key).unwrap();
        assert!(auth.verify(&token).is_some());
        assert!(auth.logout(&token));
        assert!(auth.verify(&token).is_none());
    }

    #[test]
    fn invalid_config_rejected() {
        let bad = AuthConfig {
            enabled: true,
            session_ttl_secs: 60,
            keys: vec![ApiKeyEntry {
                name: "bad".into(),
                role: "admin".into(),
                key_hash: "zu-kurz".into(),
            }],
        };
        assert!(Auth::new(&bad).is_err());

        let empty = AuthConfig {
            enabled: true,
            session_ttl_secs: 60,
            keys: vec![],
        };
        assert!(Auth::new(&empty).is_err());
    }

    #[test]
    fn generated_keys_are_unique() {
        let (key1, hash1) = generate_key();
        let (key2, hash2) = generate_key();
        assert_ne!(key1, key2);
        assert_ne!(hash1, hash2);
        assert!(key1.starts_with("bxk-"));
        assert!(verify_key_hash(&key1, &hash1));
    }
}
