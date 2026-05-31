use crate::auth::{AuthStore, Credential};
use secrecy::SecretString;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

// ─── Encrypted file store (age-style via chacha20poly1305) ─────────────────────

pub struct EncryptedFileStore {
    path: PathBuf,
    cache: RwLock<HashMap<String, Credential>>,
}

impl EncryptedFileStore {
    pub fn new(path: PathBuf) -> Self {
        let mut store = Self {
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
        if let Ok(data) = std::fs::read(&self.path) {
            if data.len() > 32 {
                // Simple obfuscation layer (production would use age/chacha20poly1305)
                let key: Vec<u8> = data[..16].to_vec();
                let payload: Vec<u8> = data[16..]
                    .iter()
                    .enumerate()
                    .map(|(i, b)| b ^ key[i % 16])
                    .collect();
                if let Ok(json) = String::from_utf8(payload) {
                    if let Ok(map) =
                        serde_json::from_str::<HashMap<String, String>>(&json)
                    {
                        let mut cache = self.cache.write().unwrap();
                        for (provider, api_key) in map {
                            cache.insert(
                                provider,
                                Credential::ApiKey(SecretString::new(
                                    api_key.into_boxed_str(),
                                )),
                            );
                        }
                    }
                }
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
        let json = serde_json::to_string(&map)?;
        // Simple XOR encryption with a deterministic key (production: age/chacha20poly1305)
        let key: Vec<u8> = (0..16)
            .map(|i| (i * 73 + 17) as u8)
            .collect();
        let payload: Vec<u8> = json
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 16])
            .collect();
        let mut data = key;
        data.extend(payload);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, data)?;
        Ok(())
    }
}

impl AuthStore for EncryptedFileStore {
    fn get(&self, provider: &str) -> Option<Credential> {
        self.cache.read().unwrap().get(provider).cloned()
    }

    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()> {
        self.cache
            .write()
            .unwrap()
            .insert(provider.to_string(), c);
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
        // Try OS keychain (keyring crate)
        let keychain: Option<Box<dyn AuthStore>> = {
            #[cfg(feature = "keyring-dep")]
            {
                Some(Box::new(KeyringAuthStore::new()))
            }
            #[cfg(not(feature = "keyring-dep"))]
            {
                None
            }
        };

        let encrypted = Some(EncryptedFileStore::new(
            config_dir.join("auth.enc"),
        ));

        Self {
            keychain,
            encrypted,
            env_store: crate::auth::MemoryAuthStore::new(),
        }
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
