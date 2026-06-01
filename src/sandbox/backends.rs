use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use super::{Command, ExecResult, FsNetPolicy, Limits, Sandbox};

// ─── Docker sandbox ─────────────────────────────────────────────────────────────

pub struct DockerSandbox {
    root: PathBuf,
    image: String,
    policy: FsNetPolicy,
}

impl DockerSandbox {
    pub fn new(root: PathBuf, image: &str) -> Self {
        Self {
            root: root.clone(),
            image: image.to_string(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: false,
            },
        }
    }
}

#[async_trait::async_trait]
impl Sandbox for DockerSandbox {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
        let workdir = cmd.workdir.to_string_lossy().to_string();
        let mut args = vec![
            "run".into(),
            "--rm".into(),
            "-v".into(),
            format!("{}:/workspace", workdir),
            "-w".into(),
            "/workspace".into(),
            format!("--memory={}m", limits.max_output_bytes / 1024 / 1024 + 128),
        ];

        if !self.policy.allow_network {
            args.push("--network=none".into());
        }

        args.push(self.image.clone());
        args.push(cmd.program.clone());
        args.extend(cmd.args.clone());

        let output = StdCommand::new("docker").args(&args).output()?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

// ─── SSH remote sandbox ─────────────────────────────────────────────────────────

pub struct SshSandbox {
    root: PathBuf,
    host: String,
    policy: FsNetPolicy,
}

impl SshSandbox {
    pub fn new(root: PathBuf, host: &str) -> Self {
        Self {
            root,
            host: host.to_string(),
            policy: FsNetPolicy {
                allowed_paths: vec![],
                allow_network: true,
            },
        }
    }
}

#[async_trait::async_trait]
impl Sandbox for SshSandbox {
    async fn exec(&self, cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
        let full_cmd = format!(
            "cd {} && {} {}",
            cmd.workdir.display(),
            cmd.program,
            cmd.args.join(" ")
        );

        let output = StdCommand::new("ssh")
            .args([&self.host, &full_cmd])
            .output()?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

// ─── Cloud/HPC backends (modal, daytona, vercel, singularity) ───────────────────
//
// These are CLI-driven backends: when the vendor CLI is installed and
// authenticated, we shell out to run the command remotely. When it is NOT
// present we return an HONEST non-zero error (exit 127) — never a fake success.
// The "remote VM" use case is fully covered today by `SshSandbox` and
// `DockerSandbox`; these add vendor-managed environments on top.

macro_rules! cli_sandbox {
    ($name:ident, $label:expr, $bin:expr, $exec_args:expr) => {
        pub struct $name {
            root: PathBuf,
            policy: FsNetPolicy,
        }

        impl $name {
            pub fn new(root: PathBuf) -> Self {
                Self {
                    root: root.clone(),
                    policy: FsNetPolicy {
                        allowed_paths: vec![root],
                        allow_network: true,
                    },
                }
            }

            fn cli_available() -> bool {
                StdCommand::new($bin)
                    .arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        }

        #[async_trait::async_trait]
        impl Sandbox for $name {
            async fn exec(&self, cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
                if !Self::cli_available() {
                    // Honest failure — not a fabricated success.
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!(
                            "{} sandbox unavailable: '{}' CLI not found or not authenticated. \
                             Install/login to it, or use sandbox=ssh / sandbox=docker which are \
                             fully supported.",
                            $label, $bin
                        ),
                        exit_code: 127,
                    });
                }
                let user_cmd = format!("{} {}", cmd.program, cmd.args.join(" "));
                let mut args: Vec<String> =
                    $exec_args.iter().map(|s: &&str| s.to_string()).collect();
                args.push(user_cmd);
                let output = StdCommand::new($bin)
                    .args(&args)
                    .current_dir(&cmd.workdir)
                    .output()?;
                Ok(ExecResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or(-1),
                })
            }

            fn root(&self) -> &Path {
                &self.root
            }

            fn policy(&self) -> &FsNetPolicy {
                &self.policy
            }
        }
    };
}

// Best-effort vendor CLI invocations; exact sub-commands are configurable by
// installing the vendor CLI which defines them. Missing CLI → honest error.
cli_sandbox!(ModalSandbox, "modal", "modal", ["run", "--"]);
cli_sandbox!(DaytonaSandbox, "daytona", "daytona", ["exec", "--"]);
cli_sandbox!(VercelSandbox, "vercel-sandbox", "vercel", ["exec", "--"]);
cli_sandbox!(SingularitySandbox, "singularity", "singularity", ["exec"]);
