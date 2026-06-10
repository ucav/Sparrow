// ─── Self-update — check, notify, and install updates ─────────────────────────
//
// Checks GitHub releases and crates.io for newer versions.
// Exposes check_update() for background checks and self_update() for one-click.
// Emits UpdateAvailable events for WebView and TUI consumption.

use serde::{Deserialize, Serialize};

// ─── Update info ───────────────────────────────────────────────────────────────

/// Structured update information for all surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub is_newer: bool,
    pub download_url: Option<String>,
    pub crate_url: String,
    pub release_url: String,
    pub install_cmd: String,
}

impl UpdateInfo {
    pub fn up_to_date(current: &str) -> Self {
        UpdateInfo {
            current: current.to_string(),
            latest: current.to_string(),
            is_newer: false,
            download_url: None,
            crate_url: format!("https://crates.io/crates/sparrow-cli"),
            release_url: format!("https://github.com/ucav/Sparrow/releases"),
            install_cmd: "cargo install sparrow-cli".to_string(),
        }
    }
}

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_API: &str = "https://api.github.com/repos/ucav/Sparrow/releases/latest";
const CRATES_API: &str = "https://crates.io/api/v1/crates/sparrow-cli";

// ─── Public API ────────────────────────────────────────────────────────────────

/// Check for updates (blocking — call in background for UI).
/// Returns Some(UpdateInfo) if a newer version is available.
pub fn check_update() -> Option<UpdateInfo> {
    // Try GitHub first (fastest, includes binary download URLs)
    if let Some(info) = check_github() {
        if info.is_newer {
            return Some(info);
        }
    }

    // Fallback to crates.io
    if let Some(info) = check_cratesio() {
        if info.is_newer {
            return Some(info);
        }
    }

    None
}

/// Perform self-update by downloading and replacing the current binary.
/// Only works when running from a release binary (not cargo run).
pub fn self_update() -> anyhow::Result<String> {
    let current = CURRENT_VERSION;

    // Find latest version
    let latest = match check_github() {
        Some(info) if info.is_newer => info.latest,
        _ => match check_cratesio() {
            Some(info) if info.is_newer => info.latest,
            _ => return Ok(format!("Already up to date (v{}). 🐦", current)),
        },
    };

    let platform = if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "macos") {
        "macos-arm64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64.exe"
    } else {
        anyhow::bail!("Unsupported platform for auto-update. Use: cargo install sparrow-cli");
    };

    let download_url = format!(
        "https://github.com/ucav/Sparrow/releases/download/v{}/sparrow-{}",
        latest, platform
    );

    let bin_path = std::env::current_exe()?;
    let new_bin = bin_path.with_extension("new");

    // Download
    let client = reqwest::blocking::Client::builder()
        .user_agent("sparrow-updater")
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let response = client.get(&download_url).send()?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Download failed ({}). Try: {}",
            response.status(),
            "cargo install sparrow-cli"
        );
    }

    let bytes = response.bytes()?;
    std::fs::write(&new_bin, &bytes)?;

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
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms)?;
    }

    Ok(format!(
        "Updated from v{} → v{}. Restart Sparrow to apply. 🐦",
        current, latest
    ))
}

// ─── Backends ──────────────────────────────────────────────────────────────────

fn check_github() -> Option<UpdateInfo> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("sparrow-update-check")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp: serde_json::Value = client.get(GITHUB_API).send().ok()?.json().ok()?;

    let latest = resp["tag_name"]
        .as_str()
        .unwrap_or("v0.0.0")
        .trim_start_matches('v');

    let is_newer = is_newer_version(latest, CURRENT_VERSION);

    // Build download URL for current platform
    let platform = if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "macos") {
        "macos-arm64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64.exe"
    } else {
        return Some(UpdateInfo {
            current: CURRENT_VERSION.to_string(),
            latest: latest.to_string(),
            is_newer,
            download_url: None,
            crate_url: "https://crates.io/crates/sparrow-cli".to_string(),
            release_url: format!("https://github.com/ucav/Sparrow/releases/tag/v{}", latest),
            install_cmd: "cargo install sparrow-cli".to_string(),
        });
    };

    Some(UpdateInfo {
        current: CURRENT_VERSION.to_string(),
        latest: latest.to_string(),
        is_newer,
        download_url: Some(format!(
            "https://github.com/ucav/Sparrow/releases/download/v{}/sparrow-{}",
            latest, platform
        )),
        crate_url: "https://crates.io/crates/sparrow-cli".to_string(),
        release_url: format!("https://github.com/ucav/Sparrow/releases/tag/v{}", latest),
        install_cmd: "cargo install sparrow-cli".to_string(),
    })
}

fn check_cratesio() -> Option<UpdateInfo> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("sparrow-update-check (crates.io)")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp: serde_json::Value = client.get(CRATES_API).send().ok()?.json().ok()?;

    let latest = resp["crate"]["max_stable_version"]
        .as_str()
        .or_else(|| resp["crate"]["max_version"].as_str())
        .unwrap_or("0.0.0");

    let is_newer = is_newer_version(latest, CURRENT_VERSION);

    Some(UpdateInfo {
        current: CURRENT_VERSION.to_string(),
        latest: latest.to_string(),
        is_newer,
        download_url: None,
        crate_url: "https://crates.io/crates/sparrow-cli".to_string(),
        release_url: "https://github.com/ucav/Sparrow/releases".to_string(),
        install_cmd: "cargo install sparrow-cli".to_string(),
    })
}

// ─── Version comparison ────────────────────────────────────────────────────────

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };

    let latest_parts = parse(latest);
    let current_parts = parse(current);

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    // If all matching parts are equal, the one with more parts is newer
    // e.g., 0.7.1 > 0.7.0, but 0.7.0-rc1 is NOT > 0.7.0
    if latest_parts.len() > current_parts.len() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.7.1", "0.7.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("0.8.0", "0.7.9"));
        assert!(!is_newer_version("0.7.0", "0.7.0"));
        assert!(!is_newer_version("0.6.9", "0.7.0"));
        assert!(!is_newer_version("0.7.0", "0.7.1"));
    }

    #[test]
    fn test_version_with_prefix() {
        assert!(is_newer_version("v0.7.1", "0.7.0"));
        assert!(is_newer_version("v1.0.0", "v0.9.9"));
    }

    #[test]
    fn test_update_info_up_to_date() {
        let info = UpdateInfo::up_to_date("0.7.0");
        assert!(!info.is_newer);
        assert_eq!(info.current, info.latest);
    }
}
