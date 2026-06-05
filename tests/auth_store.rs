use sparrow::auth::store::EncryptedFileStore;
use sparrow::auth::{AuthStore, Credential};

fn temp_auth_path(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("sparrow-{name}-{id}"))
        .join("auth.enc")
}

#[test]
fn encrypted_file_store_does_not_persist_plaintext_secret() {
    let path = temp_auth_path("auth-encrypted");
    let secret = "sk-test-secret-not-in-file";
    let store = EncryptedFileStore::new(path.clone());
    store
        .set("openai", Credential::api_key(secret))
        .expect("store credential");

    let bytes = std::fs::read(&path).expect("auth.enc exists");
    assert!(
        bytes.starts_with(b"SPARROW-AUTH-V1\n"),
        "auth file must use encrypted v1 envelope"
    );
    assert!(
        !String::from_utf8_lossy(&bytes).contains(secret),
        "auth file must not contain plaintext API keys"
    );

    let key_path = path.with_extension("key");
    let key = std::fs::read(&key_path).expect("auth key exists");
    assert_eq!(key.len(), 32, "ChaCha20-Poly1305 key must be 32 bytes");

    let reopened = EncryptedFileStore::new(path.clone());
    let loaded = reopened
        .get("openai")
        .and_then(|credential| credential.expose_key().map(str::to_string));
    assert_eq!(loaded.as_deref(), Some(secret));

    let root = path.parent().expect("parent").to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn encrypted_file_store_migrates_plain_json_on_next_save() {
    let path = temp_auth_path("auth-migrate-json");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, br#"{"nvidia":"nvapi-old-json"}"#).expect("write legacy json");

    let store = EncryptedFileStore::new(path.clone());
    let loaded = store
        .get("nvidia")
        .and_then(|credential| credential.expose_key().map(str::to_string));
    assert_eq!(loaded.as_deref(), Some("nvapi-old-json"));

    store
        .set("openrouter", Credential::api_key("or-new-secret"))
        .expect("save migrated file");
    let bytes = std::fs::read(&path).expect("auth.enc exists");
    assert!(bytes.starts_with(b"SPARROW-AUTH-V1\n"));
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains("nvapi-old-json"));
    assert!(!body.contains("or-new-secret"));

    let reopened = EncryptedFileStore::new(path.clone());
    assert_eq!(
        reopened
            .get("nvidia")
            .and_then(|credential| credential.expose_key().map(str::to_string))
            .as_deref(),
        Some("nvapi-old-json")
    );
    assert_eq!(
        reopened
            .get("openrouter")
            .and_then(|credential| credential.expose_key().map(str::to_string))
            .as_deref(),
        Some("or-new-secret")
    );

    let root = path.parent().expect("parent").to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}
