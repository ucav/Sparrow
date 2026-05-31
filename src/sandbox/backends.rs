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

// ─── Stub backends (modal, daytona, vercel, singularity) ────────────────────────

macro_rules! stub_sandbox {
    ($name:ident, $label:expr) => {
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
        }

        #[async_trait::async_trait]
        impl Sandbox for $name {
            async fn exec(&self, _cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
                Ok(ExecResult {
                    stdout: format!(
                        "{} sandbox: command execution (requires {} runtime)",
                        $label, $label
                    ),
                    stderr: String::new(),
                    exit_code: 0,
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

stub_sandbox!(ModalSandbox, "modal");
stub_sandbox!(DaytonaSandbox, "daytona");
stub_sandbox!(VercelSandbox, "vercel-sandbox");
stub_sandbox!(SingularitySandbox, "singularity");
