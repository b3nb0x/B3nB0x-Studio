use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub name: String,
    pub role: String,
    /// SHA-256-Hash des Keys (Hex, 64 Zeichen) - niemals der Klartext-Key.
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

/// Erzeugt den SHA-256-Hash eines API-Keys (Hex).
pub fn hash_key(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    format!("{digest:x}")
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
            let valid = entry.key_hash.len() == 64
                && entry.key_hash.chars().all(|c| c.is_ascii_hexdigit());
            if !valid {
                return Err(format!(
                    "Key '{}' hat einen ungueltigen Hash (erwartet: 64 Hex-Zeichen). Nutze --generate-key.",
                    entry.name
                ));
            }
            keys.push((
                entry.key_hash.to_lowercase(),
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
        let hash = hash_key(api_key);
        self.keys
            .iter()
            .find(|(key_hash, _)| *key_hash == hash)
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

/// Generiert einen neuen API-Key mit zugehoerigem Hash fuer die Config.
pub fn generate_key() -> (String, String) {
    let key = format!("bxk-{}", uuid::Uuid::new_v4());
    let hash = hash_key(&key);
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
        assert_eq!(hash1, hash_key(&key1));
    }
}
