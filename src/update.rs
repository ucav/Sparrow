// ─── Self-update ────────────────────────────────────────────────────────────────

pub fn self_update() -> anyhow::Result<String> {
    self_update_inner(false)
}

pub fn check_update() -> anyhow::Result<String> {
    self_update_inner(true)
}

fn self_update_inner(check_only: bool) -> anyhow::Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let bin_path = std::env::current_exe()?;

    // Check latest version from GitHub releases
    let client = reqwest::blocking::Client::builder()
        .user_agent("sparrow-updater")
        .build()?;

    let resp: serde_json::Value = client
        .get("https://api.github.com/repos/sparrow-dev/sparrow/releases/latest")
        .send()
        .map_err(|_| anyhow::anyhow!("Cannot reach GitHub. Check your connection."))?
        .json()?;

    let latest = resp["tag_name"]
        .as_str()
        .unwrap_or("v0.0.0")
        .trim_start_matches('v');

    if latest <= current {
        return Ok(format!("Already up to date (v{}).", current));
    }

    // Download new binary
    let platform = if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "macos") {
        "macos-arm64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64.exe"
    } else {
        anyhow::bail!("Unsupported platform for auto-update");
    };

    let download_url = format!(
        "https://github.com/sparrow-dev/sparrow/releases/download/v{}/sparrow-{}",
        latest, platform
    );

    let new_bin = bin_path.with_extension("new");

    let response = reqwest::blocking::get(&download_url)?;
    let bytes = response.bytes()?;
    std::fs::write(&new_bin, bytes)?;

    // Replace current binary
    #[cfg(windows)]
    {
        let old_bin = bin_path.with_extension("old");
        std::fs::rename(&bin_path, &old_bin)?;
        std::fs::rename(&new_bin, &bin_path)?;
        let _ = std::fs::remove_file(&old_bin);
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&new_bin, &bin_path)?;
        // Make executable
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms)?;
    }

    Ok(format!(
        "Updated from v{} → v{}. Restart sparrow.",
        current, latest
    ))
}

/// Check if an update is available
pub fn check_update() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let client = reqwest::blocking::Client::builder()
        .user_agent("sparrow-check")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp: serde_json::Value = client
        .get("https://api.github.com/repos/sparrow-dev/sparrow/releases/latest")
        .send()
        .ok()?
        .json()
        .ok()?;

    let latest = resp["tag_name"]
        .as_str()
        .unwrap_or("v0.0.0")
        .trim_start_matches('v');

    if latest > current {
        Some(format!("v{} available (current: v{})", latest, current))
    } else {
        None
    }
}
