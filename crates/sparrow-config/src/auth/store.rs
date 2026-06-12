use crate::auth::{AuthStore, Credential};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use secrecy::SecretString;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

// ─── On-disk credential file ──────────────────────────────────────────────────
//
// ChainedAuthStore prefers the OS keychain when available, then falls back to
// this ChaCha20-Poly1305 encrypted file. The data key is stored separately with
// restrictive permissions so the auth payload is never persisted as clear JSON.

const AUTH_MAGIC: &[u8] = b"SPARROW-AUTH-V1\n";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

pub struct EncryptedFileStore {
    path: PathBuf,
    key_path: PathBuf,
    cache: RwLock<HashMap<String, Credential>>,
}

impl EncryptedFileStore {
    pub fn new(path: PathBuf) -> Self {
        let key_path = path.with_extension("key");
        let store = Self {
            path,
            key_path,
            cache: RwLock::new(HashMap::new()),
        };
        store.load_from_file();
        store
    }

    fn load_from_file(&self) {
        if !self.path.exists() {
            return;
        }
        let Ok(data) = std::fs::read(&self.path) else {
            return;
        };
        let parsed: Option<HashMap<String, String>> = self
            .decrypt_payload(&data)
            .ok()
            .and_then(|plain| serde_json::from_slice::<HashMap<String, String>>(&plain).ok())
            // Migration path for the previous honest-but-plain JSON fallback.
            .or_else(|| serde_json::from_slice::<HashMap<String, String>>(&data).ok())
            // Migration path for the old hardcoded-XOR obfuscation.
            .or_else(|| legacy_xor_decode(&data));
        if let Some(map) = parsed {
            let mut cache = self.cache.write().unwrap();
            for (provider, api_key) in map {
                cache.insert(
                    provider,
                    Credential::ApiKey(SecretString::new(api_key.into_boxed_str())),
                );
            }
        }
    }

    fn save_to_file(&self) -> anyhow::Result<()> {
        let cache = self.cache.read().unwrap();
        let mut map = HashMap::new();
        for (provider, cred) in cache.iter() {
            if let Some(key) = cred.expose_key() {
                map.insert(provider.clone(), key.to_string());
            }
        }
        let json = serde_json::to_vec(&map)?;
        let encrypted = self.encrypt_payload(&json)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic-ish write: create tmp, set perms, rename.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &encrypted)?;
        restrict_perms(&tmp)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn encrypt_payload(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let key = self.load_or_create_key()?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let mut nonce = [0_u8; NONCE_LEN];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|err| anyhow::anyhow!("auth file encryption failed: {}", err))?;
        let mut out = Vec::with_capacity(AUTH_MAGIC.len() + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(AUTH_MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decrypt_payload(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if !data.starts_with(AUTH_MAGIC) {
            anyhow::bail!("auth file is not encrypted envelope v1");
        }
        let body = &data[AUTH_MAGIC.len()..];
        if body.len() <= NONCE_LEN {
            anyhow::bail!("auth file encrypted envelope is truncated");
        }
        let (nonce, ciphertext) = body.split_at(NONCE_LEN);
        let key = self.load_or_create_key()?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|err| anyhow::anyhow!("auth file decryption failed: {}", err))
    }

    fn load_or_create_key(&self) -> anyhow::Result<[u8; KEY_LEN]> {
        if self.key_path.exists() {
            let bytes = std::fs::read(&self.key_path)?;
            if bytes.len() != KEY_LEN {
                anyhow::bail!(
                    "auth file key has invalid length: {} bytes at {}",
                    bytes.len(),
                    self.key_path.display()
                );
            }
            let mut key = [0_u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        let mut key = [0_u8; KEY_LEN];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
        if let Some(parent) = self.key_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.key_path.with_extension("key.tmp");
        std::fs::write(&tmp, key)?;
        restrict_perms(&tmp)?;
        std::fs::rename(&tmp, &self.key_path)?;
        Ok(key)
    }
}

#[cfg(unix)]
fn restrict_perms(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_perms(_path: &std::path::Path) -> anyhow::Result<()> {
    // Windows: rely on the user's profile ACLs (file lives under %APPDATA%).
    Ok(())
}

/// Best-effort migration: decode the old XOR-obfuscated format so users who
/// already wrote credentials with the previous version are not locked out.
/// Once loaded, the next `save_to_file()` rewrites in plain JSON.
fn legacy_xor_decode(data: &[u8]) -> Option<HashMap<String, String>> {
    if data.len() <= 32 {
        return None;
    }
    let key = &data[..16];
    let payload: Vec<u8> = data[16..]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 16])
        .collect();
    let json = String::from_utf8(payload).ok()?;
    serde_json::from_str::<HashMap<String, String>>(&json).ok()
}

impl AuthStore for EncryptedFileStore {
    fn get(&self, provider: &str) -> Option<Credential> {
        self.cache.read().unwrap().get(provider).cloned()
    }

    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()> {
        self.cache.write().unwrap().insert(provider.to_string(), c);
        self.save_to_file()
    }

    fn list(&self) -> Vec<String> {
        self.cache.read().unwrap().keys().cloned().collect()
    }

    fn remove(&self, provider: &str) -> anyhow::Result<()> {
        self.cache.write().unwrap().remove(provider);
        self.save_to_file()
    }
}

// ─── Chained auth store (keychain → encrypted file → env) ──────────────────────

/// Implements the priority chain from §3.2:
/// OS keychain → encrypted file → env
pub struct ChainedAuthStore {
    keychain: Option<Box<dyn AuthStore>>,
    encrypted: Option<EncryptedFileStore>,
    env_store: crate::auth::MemoryAuthStore,
}

impl ChainedAuthStore {
    pub fn new(config_dir: PathBuf) -> Self {
        // Try OS keychain (keyring crate). The optional `keyring` dep auto-creates
        // a feature of the same name; gate on it directly.
        let keychain: Option<Box<dyn AuthStore>> = {
            #[cfg(feature = "keyring")]
            {
                Some(Box::new(KeyringAuthStore::new()))
            }
            #[cfg(not(feature = "keyring"))]
            {
                None
            }
        };

        let encrypted = Some(EncryptedFileStore::new(config_dir.join("auth.enc")));

        Self {
            keychain,
            encrypted,
            env_store: crate::auth::MemoryAuthStore::new(),
        }
    }
}

// ─── OS keychain backend ───────────────────────────────────────────────────────

#[cfg(feature = "keyring")]
pub struct KeyringAuthStore {
    service: String,
    // Cache of known provider names — keyring crates have no enumerate API.
    index: RwLock<std::collections::BTreeSet<String>>,
}

#[cfg(feature = "keyring")]
impl KeyringAuthStore {
    pub fn new() -> Self {
        Self {
            service: "sparrow".to_string(),
            index: RwLock::new(std::collections::BTreeSet::new()),
        }
    }

    fn entry(&self, provider: &str) -> keyring::Result<keyring::Entry> {
        keyring::Entry::new(&self.service, provider)
    }
}

#[cfg(feature = "keyring")]
impl AuthStore for KeyringAuthStore {
    fn get(&self, provider: &str) -> Option<Credential> {
        let entry = self.entry(provider).ok()?;
        let secret = entry.get_password().ok()?;
        self.index.write().unwrap().insert(provider.to_string());
        Some(Credential::ApiKey(SecretString::new(
            secret.into_boxed_str(),
        )))
    }

    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()> {
        let Some(key) = c.expose_key() else {
            anyhow::bail!("keyring backend only supports api-key credentials");
        };
        let entry = self
            .entry(provider)
            .map_err(|e| anyhow::anyhow!("keyring entry: {}", e))?;
        entry
            .set_password(&key)
            .map_err(|e| anyhow::anyhow!("keyring set: {}", e))?;
        self.index.write().unwrap().insert(provider.to_string());
        Ok(())
    }

    fn list(&self) -> Vec<String> {
        self.index.read().unwrap().iter().cloned().collect()
    }

    fn remove(&self, provider: &str) -> anyhow::Result<()> {
        let entry = self
            .entry(provider)
            .map_err(|e| anyhow::anyhow!("keyring entry: {}", e))?;
        // delete_credential() is best-effort: a missing entry is not an error.
        let _ = entry.delete_credential();
        self.index.write().unwrap().remove(provider);
        Ok(())
    }
}

impl AuthStore for ChainedAuthStore {
    fn get(&self, provider: &str) -> Option<Credential> {
        // Priority: keychain → encrypted file → env
        if let Some(ref kc) = self.keychain {
            if let c @ Some(_) = kc.get(provider) {
                return c;
            }
        }
        if let Some(ref enc) = self.encrypted {
            if let c @ Some(_) = enc.get(provider) {
                return c;
            }
        }
        self.env_store.get(provider)
    }

    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()> {
        // Store in encrypted file (keychain if available)
        if let Some(ref kc) = self.keychain {
            kc.set(provider, c.clone())?;
        }
        if let Some(ref enc) = self.encrypted {
            enc.set(provider, c)?;
        }
        Ok(())
    }

    fn list(&self) -> Vec<String> {
        let mut all = Vec::new();
        if let Some(ref kc) = self.keychain {
            all.extend(kc.list());
        }
        if let Some(ref enc) = self.encrypted {
            all.extend(enc.list());
        }
        all.extend(self.env_store.list());
        all.sort();
        all.dedup();
        all
    }

    fn remove(&self, provider: &str) -> anyhow::Result<()> {
        if let Some(ref kc) = self.keychain {
            kc.remove(provider)?;
        }
        if let Some(ref enc) = self.encrypted {
            enc.remove(provider)?;
        }
        Ok(())
    }
}
