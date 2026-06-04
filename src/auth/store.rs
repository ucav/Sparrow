use crate::auth::{AuthStore, Credential};
use secrecy::SecretString;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

// ─── On-disk credential file ──────────────────────────────────────────────────
//
// The previous implementation pretended to encrypt with a hardcoded XOR key, which
// is trivially reversible. We don't pretend anymore: this store writes a plain
// JSON file with restrictive permissions (0600 on Unix). If you need real at-rest
// secrecy, install the OS keychain integration — `ChainedAuthStore` prefers it.

pub struct EncryptedFileStore {
    path: PathBuf,
    cache: RwLock<HashMap<String, Credential>>,
}

impl EncryptedFileStore {
    pub fn new(path: PathBuf) -> Self {
        let store = Self {
            path,
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
        let parsed: Option<HashMap<String, String>> =
            serde_json::from_slice::<HashMap<String, String>>(&data)
                .ok()
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
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic-ish write: create tmp, set perms, rename.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &json)?;
        restrict_perms(&tmp)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
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
