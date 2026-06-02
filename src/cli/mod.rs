use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sparrow", about = "one cli · grows with you", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Launch terminal TUI (native)
    #[arg(long)]
    pub tui: bool,

    /// Launch webview console (HTTP + WebSocket)
    #[arg(long)]
    pub web: bool,

    /// JSON output (NDJSON event stream)
    #[arg(long)]
    pub json: bool,

    /// Override autonomy level
    #[arg(long)]
    pub autonomy: Option<String>,

    /// Force a specific model
    #[arg(long)]
    pub model: Option<String>,

    /// Prefer local/offline models
    #[arg(long)]
    pub local: bool,

    /// Session budget cap (USD)
    #[arg(long)]
    pub budget: Option<f64>,

    /// Sandbox backend
    #[arg(long)]
    pub sandbox: Option<String>,

    /// Profile name
    #[arg(long)]
    pub profile: Option<String>,

    /// Disable checkpointing
    #[arg(long)]
    pub no_checkpoint: bool,

    /// Run as a named agent
    #[arg(long)]
    pub agent: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a single agentic task
    Run {
        /// Task description
        task: String,

        /// Emit NDJSON event stream (same as the global --json flag, but may
        /// follow the task: `sparrow run "..." --json`)
        #[arg(long)]
        json: bool,
    },

    /// Interactive multi-turn chat
    Chat,

    /// Launch TUI
    Tui,

    /// Launch webview console (HTTP + WebSocket)
    Console,

    /// Run headless Sparrow runtime daemon
    Daemon,

    /// Manage persistent agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Run swarm: planner → coder → verifier
    Swarm {
        /// Task or plan file
        task: String,
    },

    /// Schedule periodic jobs
    Schedule {
        /// Task description
        task: String,

        /// Cron expression
        #[arg(long)]
        cron: String,

        /// Autonomy level for scheduled jobs
        #[arg(long)]
        autonomy: Option<String>,

        /// Report to surfaces
        #[arg(long)]
        report: Vec<String>,
    },

    /// Manage model routing
    Model {
        /// Set active route
        #[arg(long)]
        set: Option<String>,

        /// List available models
        #[arg(long)]
        list: bool,
    },

    /// Manage provider credentials
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Manage skill library
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },

    /// Manage MCP connectors
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// List checkpoints
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// Rewind to a checkpoint
    Rewind {
        /// Checkpoint ID or number
        id: String,
    },

    /// Replay a transcript
    Replay {
        /// Run ID to replay
        run_id: String,
        /// Open an interactive TUI scrubber (←/→ to step through events)
        #[arg(long)]
        scrub: bool,
    },

    /// Start/stop gateway daemon
    Gateway {
        #[command(subcommand)]
        action: GatewayAction,
    },

    /// Interactive tutorial
    Learn,

    /// Initialize a project with .sparrow/ config
    Init,

    /// Show live status (active runs, budget, session)
    Status,

    /// Manage persistent memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Migrate from OpenClaw
    Import {
        #[command(subcommand)]
        source: ImportSource,
    },

    /// Edit configuration
    Config {
        /// Open config.toml in editor
        #[arg(short)]
        edit: bool,
    },

    /// Self-update
    Update,

    /// Run diagnostics
    Doctor,

    /// (Re)run conversational setup
    Setup,
}

#[derive(Subcommand)]
pub enum AgentAction {
    Create { name: String },
    List,
    Edit { name: String },
    Rm { name: String },
    Run { name: String, task: String },
}

#[derive(Subcommand)]
pub enum AuthAction {
    Add {
        provider: String,
    },
    List,
    Rm {
        provider: String,
    },
    /// Authenticate a provider via OAuth device flow (github/google/microsoft).
    Login {
        provider: String,
        /// OAuth client id (or set <PROVIDER>_CLIENT_ID env var)
        #[arg(long)]
        client_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SkillsAction {
    List,
    Create { name: String },
    Prune,
    /// Remove a skill by name (e.g. to delete junk auto-learned skills)
    Rm { name: String },
}

#[derive(Subcommand)]
pub enum McpAction {
    Add {
        server: String,

        /// Command to launch the MCP server
        #[arg(long)]
        command: Option<String>,

        /// Command arguments, either repeated or space-delimited
        #[arg(long, value_delimiter = ' ', allow_hyphen_values = true)]
        args: Vec<String>,

        /// Transport backend: stdio, sse, or url
        #[arg(long)]
        transport: Option<String>,
    },
    List,
    Rm {
        server: String,
    },
}

#[derive(Subcommand)]
pub enum CheckpointAction {
    /// List all checkpoints
    List,
    /// Show diff between HEAD and a checkpoint
    Diff {
        /// Checkpoint ID
        id: String,
    },
    /// Delete checkpoints older than N days (default: 30)
    Prune {
        /// Remove checkpoints older than this many days
        #[arg(long, default_value = "30")]
        older_than_days: u64,
    },
}

#[derive(Subcommand)]
pub enum GatewayAction {
    Start,
    Status,
    Stop,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    Create { name: String },
    List,
    Use { name: String },
}

#[derive(Subcommand)]
pub enum ImportSource {
    Openclaw { path: Option<PathBuf> },
}

#[derive(Subcommand)]
pub enum MemoryAction {
    List,
    Forget { id: String },
    Add { key: String, value: String },
}
