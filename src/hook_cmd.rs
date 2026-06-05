//! `sparrow hook install` and `sparrow hook scan` commands.
//!
//! The `install` subcommand installs a pre-commit hook via `git config` that
//! scans staged files for secrets, tokens, and other sensitive content before
//! each commit.
//!
//! The `scan` subcommand performs a one-off scan of staged files (or the
//! entire working tree) for secrets and sensitive patterns.

use std::path::PathBuf;
use std::process::Command;

// ─── Hook install ────────────────────────────────────────────────────────────

/// Install the Sparrow pre-commit hook into the current git repository.
///
/// Copies `hooks/pre-commit` to `.git/hooks/pre-commit` and makes it
/// executable. If a hook already exists, it is backed up first.
pub fn run_hook_install() -> anyhow::Result<()> {
    // Find the git repository root
    let repo_root = find_git_root()?;

    let hooks_dir = repo_root.join(".git").join("hooks");
    let pre_commit_path = hooks_dir.join("pre-commit");

    // Find the Sparrow hooks directory (next to the binary, or in the project)
    let sparrow_hook_script = find_sparrow_hook_script()?;

    println!("🔒 Sparrow Hook Install");
    println!("   Dépôt  : {}", repo_root.display());
    println!("   Script : {}", sparrow_hook_script.display());

    // Create hooks dir if needed
    std::fs::create_dir_all(&hooks_dir)?;

    // Backup existing hook
    if pre_commit_path.exists() {
        let backup = hooks_dir.join("pre-commit.sparrow-backup");
        println!("   ⚠️  Un hook pre-commit existe déjà → backup → {}", backup.display());
        std::fs::rename(&pre_commit_path, &backup)?;
    }

    // Copy the hook script
    std::fs::copy(&sparrow_hook_script, &pre_commit_path)?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&pre_commit_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&pre_commit_path, perms)?;
    }

    println!("   ✓ Hook pre-commit installé !");
    println!();
    println!("   Le hook scanne automatiquement :");
    println!("   • Clés API et tokens (AWS, GitHub, OpenAI, etc.)");
    println!("   • Fichiers .env, credentials.json");
    println!("   • Fichiers d'agent IA (.sparrow/, .codex/)");
    println!("   • Secrets en clair (password, secret, token)");
    println!();
    println!("   Pour désinstaller : rm {}", pre_commit_path.display());
    println!(
        "   Pour restaurer l'ancien hook : mv {} {}",
        hooks_dir.join("pre-commit.sparrow-backup").display(),
        pre_commit_path.display(),
    );

    Ok(())
}

// ─── Hook scan ───────────────────────────────────────────────────────────────

/// Run a one-off scan of staged files (or the entire working tree if
/// `scan_all` is true) for secrets and sensitive patterns.
pub fn run_hook_scan(scan_all: bool) -> anyhow::Result<()> {
    let repo_root = match find_git_root() {
        Ok(r) => r,
        Err(_) => {
            // Not in a git repo — scan the current directory
            std::env::current_dir()?
        }
    };

    println!("🔍 Sparrow Security Scan");
    println!("   Répertoire : {}", repo_root.display());

    if scan_all {
        println!("   Mode : arbre complet");
        println!();
        scan_directory(&repo_root)?;
    } else {
        println!("   Mode : fichiers stagés (git diff --cached)");
        println!();
        scan_staged_files(&repo_root)?;
    }

    Ok(())
}

// ─── Git helpers ─────────────────────────────────────────────────────────────

fn find_git_root() -> anyhow::Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Pas dans un dépôt Git.\n\
             → Lance cette commande depuis un dépôt Git.\n\
             → Ou utilise `sparrow hook scan --all` pour scanner le dossier courant."
        );
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path))
}

fn find_sparrow_hook_script() -> anyhow::Result<PathBuf> {
    // Look in several places:
    // 1. Relative to the current executable
    // 2. In the project checkout at hooks/pre-commit
    // 3. Embedded fallback

    let candidates = vec![
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("hooks/pre-commit")))
            .unwrap_or_default(),
        PathBuf::from("hooks/pre-commit"),
        PathBuf::from("/tmp/Sparrow_cleanup/hooks/pre-commit"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // If the script file doesn't exist yet, create it from our embedded content
    let hooks_dir = PathBuf::from("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-commit");
    if !hook_path.exists() {
        std::fs::write(&hook_path, PRE_COMMIT_HOOK_SCRIPT)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&hook_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_path, perms)?;
        }
    }
    Ok(hook_path)
}

// ─── Scanning logic ──────────────────────────────────────────────────────────

/// Scan staged files (git diff --cached --name-only).
fn scan_staged_files(repo_root: &std::path::Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        // No staged files or not a git repo — scan nothing
        println!("   Aucun fichier stagé à scanner.");
        return Ok(());
    }

    let files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| repo_root.join(l))
        .collect();

    if files.is_empty() {
        println!("   ✓ Aucun fichier stagé trouvé.");
        return Ok(());
    }

    println!("   {} fichier(s) à scanner...\n", files.len());

    let mut issues_found = 0;

    for file in &files {
        if let Ok(content) = std::fs::read_to_string(file) {
            let findings = scan_content(&content, file);
            if !findings.is_empty() {
                for finding in findings {
                    println!("   ⚠️  {} : {}", file.display(), finding);
                    issues_found += 1;
                }
            }
        }
    }

    if issues_found == 0 {
        println!("   ✓ Aucun problème détecté !");
    } else {
        println!("\n   ⚠️  {} problème(s) détecté(s) !", issues_found);
        println!("   → Corrige-les avant de commit.\n");
        // Exit with non-zero to block the commit
        std::process::exit(1);
    }

    Ok(())
}

/// Scan all files in a directory recursively.
fn scan_directory(dir: &std::path::Path) -> anyhow::Result<()> {
    

    let mut issues_found = 0;
    let mut files_scanned = 0;

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip hidden directories (but not hidden files at root)
        if path
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.') && c != path.components().next().unwrap_or(std::path::Component::CurDir))
        {
            // Skip .git, .sparrow, node_modules, target, etc.
            let path_str = path.to_string_lossy();
            if path_str.contains("/.git/")
                || path_str.contains("/node_modules/")
                || path_str.contains("/target/")
                || path_str.contains("/.sparrow/")
                || path_str.contains("/__pycache__/")
            {
                continue;
            }
        }

        // Skip binary files (simple heuristic)
        if let Ok(content) = std::fs::read(path) {
            if content.iter().any(|&b| b == 0) {
                continue; // null byte → binary
            }
        }

        files_scanned += 1;

        if let Ok(content) = std::fs::read_to_string(path) {
            let findings = scan_content(&content, path);
            for finding in findings {
                println!("   ⚠️  {} : {}", path.display(), finding);
                issues_found += 1;
            }
        }
    }

    println!("\n   {} fichiers scannés.", files_scanned);
    if issues_found == 0 {
        println!("   ✓ Aucun problème détecté !");
    } else {
        println!("   ⚠️  {} problème(s) détecté(s) !", issues_found);
    }

    Ok(())
}

// ─── Secret patterns ─────────────────────────────────────────────────────────

/// Patterns that indicate a potential secret leak.
static SECRET_PATTERNS: &[(&str, &str)] = &[
    // API keys (generic)
    (r"(?i)(?:api[_-]?key|api[_-]?secret|apikey)\s*[:=]\s*[\x27\x22]?\w{20,}[\x27\x22]?", "Clé API en clair"),
    (r"(?i)(?:secret[_-]?key|secretkey)\s*[:=]\s*[\x27\x22]?\w{20,}[\x27\x22]?", "Clé secrète en clair"),
    (r"(?i)(?:access[_-]?key|accesskey)\s*[:=]\s*[\x27\x22]?\w{16,}[\x27\x22]?", "Clé d'accès en clair"),

    // AWS
    (r"AKIA[0-9A-Z]{16}", "AWS Access Key ID"),
    (r"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*[\x27\x22]?[0-9a-zA-Z/+]{40}[\x27\x22]?", "AWS Secret Key"),

    // GitHub
    (r"ghp_[0-9a-zA-Z]{36}", "GitHub Personal Access Token"),
    (r"github_pat_[0-9a-zA-Z_]{36,}", "GitHub PAT (fine-grained)"),
    (r"gho_[0-9a-zA-Z]{36}", "GitHub OAuth Token"),

    // OpenAI
    (r"sk-[0-9a-zA-Z]{32,}", "OpenAI API Key"),
    (r"sk-proj-[0-9a-zA-Z]{32,}", "OpenAI Project Key"),

    // Anthropic
    (r"sk-ant-[0-9a-zA-Z]{32,}", "Anthropic API Key"),

    // Generic token patterns
    (r"(?i)(?:password|passwd|pwd)\s*[:=]\s*[\x27\x22]?\S{4,}[\x27\x22]?", "Mot de passe en clair"),
    (r"(?i)(?:token|auth[_-]?token)\s*[:=]\s*[\x27\x22]?\S{20,}[\x27\x22]?", "Token en clair"),

    // Private keys
    (r"-----BEGIN (?:RSA|DSA|EC|OPENSSH|PGP) PRIVATE KEY-----", "Clé privée"),
    (r"-----BEGIN PRIVATE KEY-----", "Clé privée"),

    // JWT tokens
    (r"eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}", "JWT Token"),

    // Slack
    (r"xox[baprs]-[0-9a-zA-Z-]{10,}", "Slack Token"),

    // Stripe
    (r"sk_live_[0-9a-zA-Z]{24,}", "Stripe Live Key"),
    (r"pk_live_[0-9a-zA-Z]{24,}", "Stripe Live Publishable Key"),

    // Google
    (r"AIza[0-9A-Za-z_-]{35}", "Google API Key"),

    // Agent files
    (r"(?i)\.(?:sparrow|codex|agent|ai|llm)", "Fichier de config agent IA"),
];

/// Sensitive filenames that should never be committed.
static SENSITIVE_FILENAMES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    "credentials.json",
    "service-account.json",
    "secrets.yaml",
    "secrets.yml",
    ".netrc",
    ".npmrc",
    ".pypirc",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
];

/// Scan a file's content for secrets.
fn scan_content(content: &str, path: &std::path::Path) -> Vec<String> {
    let mut findings = Vec::new();

    // Check filename
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    for &sensitive in SENSITIVE_FILENAMES {
        if sensitive.starts_with('*') {
            let ext = &sensitive[1..]; // e.g., ".pem" from "*.pem"
            if filename.ends_with(ext) {
                findings.push(format!("Fichier sensible ({sensitive})"));
            }
        } else if filename == sensitive {
            findings.push(format!("Fichier sensible ({sensitive})"));
        }
    }

    // Check content patterns
    for &(pattern, label) in SECRET_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(content) {
                // Find the matching line for context
                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        // Redact the secret part for display
                        let redacted = re.replace(line, |_caps: &regex::Captures| {
                            "***REDACTED***"
                        });
                        findings.push(format!(
                            "{} (ligne {}) : {}",
                            label,
                            line_num + 1,
                            redacted.trim(),
                        ));
                        break; // One finding per pattern per file
                    }
                }
            }
        }
    }

    findings
}

// ─── Embedded pre-commit hook script (shell) ─────────────────────────────────

/// The shell script that is installed as a Git pre-commit hook.
/// It runs `sparrow hook scan` to check staged files.
pub const PRE_COMMIT_HOOK_SCRIPT: &str = r#"#!/usr/bin/env bash
# Sparrow Pre-Commit Hook
# Scans staged files for secrets, tokens, and sensitive patterns.
# Installed by: sparrow hook install
# Docs: https://github.com/ucav/Sparrow

set -euo pipefail

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔒 Sparrow pre-commit hook — scanning staged files...${NC}"

# ─── 1. Check for sensitive filenames ────────────────────────────────
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM 2>/dev/null || true)

if [ -z "$STAGED_FILES" ]; then
    echo -e "${GREEN}✓ No staged files to scan.${NC}"
    exit 0
fi

SENSITIVE_NAMES=(
    ".env" ".env.local" ".env.production" ".env.development"
    "credentials.json" "service-account.json" "secrets.yaml" "secrets.yml"
    ".netrc" ".npmrc" ".pypirc"
    "id_rsa" "id_ed25519" "id_ecdsa"
)

ISSUES=0

for file in $STAGED_FILES; do
    filename=$(basename "$file")
    for sensitive in "${SENSITIVE_NAMES[@]}"; do
        if [ "$filename" = "$sensitive" ]; then
            echo -e "${RED}✗ BLOCKED: Sensitive file staged: $file${NC}"
            echo -e "  → Remove it: git rm --cached $file"
            ISSUES=$((ISSUES + 1))
        fi
    done

    # Check for .pem, .key, .p12, .pfx extensions
    case "$filename" in
        *.pem|*.key|*.p12|*.pfx)
            echo -e "${RED}✗ BLOCKED: Private key file staged: $file${NC}"
            echo -e "  → Remove it: git rm --cached $file"
            ISSUES=$((ISSUES + 1))
            ;;
    esac

    # Check for agent config directories
    case "$file" in
        .sparrow/*|.codex/*|.agent/*)
            echo -e "${YELLOW}⚠ WARNING: Agent config directory staged: $file${NC}"
            echo -e "  → Consider adding to .gitignore"
            ;;
    esac

    # ─── 2. Scan content of staged files ─────────────────────────────
    if [ -f "$file" ]; then
        # Get the staged content (not working tree)
        STAGED_CONTENT=$(git show ":$file" 2>/dev/null || true)

        if [ -n "$STAGED_CONTENT" ]; then
            # Check for common API key patterns
            # GitHub tokens
            if echo "$STAGED_CONTENT" | grep -qE 'ghp_[0-9a-zA-Z]{36}|github_pat_[0-9a-zA-Z_]{36,}'; then
                echo -e "${RED}✗ BLOCKED: GitHub token detected in $file${NC}"
                ISSUES=$((ISSUES + 1))
            fi

            # OpenAI keys
            if echo "$STAGED_CONTENT" | grep -qE 'sk-[0-9a-zA-Z]{32,}|sk-proj-[0-9a-zA-Z]{32,}'; then
                echo -e "${RED}✗ BLOCKED: OpenAI API key detected in $file${NC}"
                ISSUES=$((ISSUES + 1))
            fi

            # Anthropic keys
            if echo "$STAGED_CONTENT" | grep -qE 'sk-ant-[0-9a-zA-Z]{32,}'; then
                echo -e "${RED}✗ BLOCKED: Anthropic API key detected in $file${NC}"
                ISSUES=$((ISSUES + 1))
            fi

            # AWS keys
            if echo "$STAGED_CONTENT" | grep -qE 'AKIA[0-9A-Z]{16}'; then
                echo -e "${RED}✗ BLOCKED: AWS Access Key detected in $file${NC}"
                ISSUES=$((ISSUES + 1))
            fi

            # Generic API key patterns
            if echo "$STAGED_CONTENT" | grep -qiE '(api[_-]?key|api[_-]?secret|secret[_-]?key)\s*[:=]\s*['"'"'"]?\w{20,}'; then
                echo -e "${YELLOW}⚠ WARNING: Possible API key in $file${NC}"
                echo -e "  → Review and use environment variables instead"
            fi

            # Private keys
            if echo "$STAGED_CONTENT" | grep -qE '-----BEGIN (RSA|DSA|EC|OPENSSH|PGP) PRIVATE KEY-----'; then
                echo -e "${RED}✗ BLOCKED: Private key detected in $file${NC}"
                ISSUES=$((ISSUES + 1))
            fi

            # Password/token assignment
            if echo "$STAGED_CONTENT" | grep -qiE '(password|passwd|pwd|token|auth[_-]?token)\s*[:=]\s*['"'"'"]?\S{4,}'; then
                echo -e "${YELLOW}⚠ WARNING: Possible hardcoded password/token in $file${NC}"
            fi
        fi
    fi
done

# ─── 3. Result ────────────────────────────────────────────────────────────────
if [ $ISSUES -gt 0 ]; then
    echo ""
    echo -e "${RED}══════════════════════════════════════════════════${NC}"
    echo -e "${RED}  COMMIT BLOQUÉ — $ISSUES problème(s) de sécurité${NC}"
    echo -e "${RED}══════════════════════════════════════════════════${NC}"
    echo ""
    echo "Pour ignorer (déconseillé) : git commit --no-verify"
    echo "Pour enlever un fichier du stage : git rm --cached <fichier>"
    echo "Pour désinstaller ce hook : rm .git/hooks/pre-commit"
    exit 1
else
    echo -e "${GREEN}✓ Aucun problème détecté — commit autorisé.${NC}"
    exit 0
fi
"#;
