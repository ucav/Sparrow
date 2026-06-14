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
    #[arg(long, global = true)]
    pub local: bool,

    /// Session budget cap (USD)
    #[arg(long, global = true)]
    pub budget: Option<f64>,

    /// Hard stop on cumulative USD spent in this run (alias for --budget,
    /// kept separately to match competitor tools' UX).
    #[arg(long, global = true)]
    pub max_cost_usd: Option<f64>,

    /// Hard stop on wall-clock seconds elapsed in this run.
    #[arg(long, global = true)]
    pub max_wall_secs: Option<u64>,

    /// Hard stop on total tokens consumed in this run.
    #[arg(long, global = true)]
    pub max_tokens: Option<u64>,

    /// Bind address for daemon / cockpit servers (default 127.0.0.1).
    /// Use 0.0.0.0 when running under WSL or in a container.
    #[arg(long, global = true)]
    pub bind: Option<String>,

    /// Sandbox backend
    #[arg(long, global = true)]
    pub sandbox: Option<String>,

    /// Profile name
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Disable checkpointing
    #[arg(long, global = true)]
    pub no_checkpoint: bool,

    /// Run as a named agent
    #[arg(long)]
    pub agent: Option<String>,

    /// Continue the most recent session (any surface) instead of this
    /// directory's session
    #[arg(long = "continue", global = true)]
    pub continue_last: bool,

    /// Start with a fresh context (ignore this directory's saved session)
    #[arg(long, global = true)]
    pub fresh: bool,

    /// Skip the pre-run quote confirmation
    #[arg(long, global = true)]
    pub yes: bool,
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

        /// Show a read-only plan first; continue only with `--yes`.
        #[arg(long)]
        plan_first: bool,

        /// Read-only dry run: propose actions/diffs, but deny mutating tools.
        #[arg(long)]
        dry_run: bool,

        /// Patch mode: ask for a unified diff only and deny mutating tools.
        #[arg(long)]
        patch: bool,
    },

    /// Create a read-only execution plan for a task
    Plan {
        /// Task description
        task: String,

        /// Emit JSON instead of Markdown
        #[arg(long)]
        json: bool,
    },

    /// Audit the current repository: architecture map, stubs, TODO/FIXME, and
    /// suspicious Rust files. Writes `./artifacts/audit-<timestamp>.md`.
    Audit {
        /// Emit JSON instead of Markdown path output
        #[arg(long)]
        json: bool,
    },

    /// Detect and run the project test suite (`cargo`, `npm`, or `pytest`).
    Test {
        /// If tests fail, hand the failure to Sparrow's repair loop.
        #[arg(long)]
        fix: bool,

        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },

    /// Adversarial review of the current local diff (uncommitted, staged,
    /// and commits ahead of `--base`). Read-only — no edits, no commits, no
    /// network beyond the model call. Findings are structured around
    /// security, correctness, regressions, performance, and readability.
    Review {
        /// Base ref to diff against (defaults to `origin/main`, then `main`,
        /// then `HEAD~1`).
        #[arg(long)]
        base: Option<String>,

        /// Only review changes touching these path globs (repeatable).
        #[arg(long)]
        paths: Vec<String>,

        /// Print the prompt the model will see and exit (no model call).
        #[arg(long)]
        dry_run: bool,
    },

    /// Interactive multi-turn chat
    Chat,

    /// Answer a reasoning-heavy task with inference-time scaling
    /// (best-of-N + judge selection + self-refine) instead of one greedy pass.
    Reason {
        /// The task / question to reason about.
        task: String,
    },

    /// Launch TUI
    Tui,

    /// Launch first-run setup, then the WebView cockpit
    Launch {
        /// TCP port for the WebView HTTP/WS server
        #[arg(long, default_value = "9339")]
        port: u16,

        /// Launch the terminal TUI instead of the WebView cockpit
        #[arg(long)]
        tui: bool,

        /// Use the older expert setup wizard before opening the surface
        #[arg(long)]
        pro: bool,
    },

    /// Create a clean git commit from staged changes after a secret scan.
    Commit {
        /// Commit message. If omitted, Sparrow generates a conservative one
        /// from the staged diff stat.
        #[arg(short, long)]
        message: Option<String>,

        /// Show what would be committed without running `git commit`.
        #[arg(long)]
        dry_run: bool,
    },

    /// Release workflow helpers.
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },

    /// Public release intelligence (opt-in network scan, local cache reports).
    Intel {
        #[command(subcommand)]
        action: IntelAction,
    },

    /// Launch webview console (HTTP + WebSocket)
    #[command(visible_aliases = ["montre", "show"])]
    Console {
        /// TCP port for the webview HTTP/WS server
        #[arg(long, default_value = "9339")]
        port: u16,

        /// Fast start: skip boot animation, eager panel preloads, and boot-time
        /// provider discovery. Panels still load lazily when opened.
        #[arg(long)]
        fast: bool,
    },

    /// Réparer un problème — décris ce qui ne va pas, Sparrow diagnostique
    /// puis corrige (avec ton accord). « sparrow fix "message d'erreur" »,
    /// ou sans argument pour scanner le dossier courant.
    #[command(visible_aliases = ["repare", "répare"])]
    Fix {
        /// Le problème, avec tes mots, ou une erreur collée (entre guillemets
        /// si elle contient des espaces). Optionnel : sans argument, Sparrow
        /// inspecte le dossier courant.
        problem: Vec<String>,
    },

    /// Dis ce que tu veux en langage naturel — Sparrow choisit la commande.
    /// « sparrow do "corrige le build" » · « sparrow do "montre la console" ».
    /// Pas besoin d'apprendre les commandes : décris, Sparrow comprend.
    #[command(visible_aliases = ["fais"])]
    Do {
        /// Ta demande, avec tes mots.
        request: Vec<String>,
        /// Montre seulement la commande choisie, sans l'exécuter.
        #[arg(long)]
        dry_run: bool,
    },

    /// Expliquer un fichier, une erreur ou un concept en langage simple.
    /// « sparrow explique src/main.rs » · « sparrow explique "borrow checker" »
    #[command(visible_aliases = ["explain"])]
    Explique {
        /// Ce qu'il faut expliquer : un chemin de fichier, une erreur, ou un
        /// mot (entre guillemets si plusieurs mots).
        target: Vec<String>,
    },

    /// Annuler la dernière action de Sparrow — revient au dernier point de
    /// sauvegarde, rien n'est perdu. « sparrow annule » · « sparrow annule
    /// --tout » pour revenir au début de la session.
    #[command(visible_aliases = ["undo"])]
    Annule {
        /// Point de sauvegarde précis (sinon : le tout dernier).
        id: Option<String>,

        /// Revenir au tout premier point de sauvegarde de la session.
        #[arg(long, visible_alias = "all")]
        tout: bool,
    },

    /// Dire bonjour — l'accueil chaleureux : Sparrow regarde ton dossier et
    /// te propose quoi faire. Parfait pour un premier contact.
    #[command(visible_aliases = ["hello", "salut"])]
    Bonjour,

    /// Voir ou changer le plafond de dépense par session. « sparrow budget »
    /// affiche le réglage actuel ; « sparrow budget 2€ » le change.
    Budget {
        /// Le montant max par session (ex. « 2€ », « $0.50 », « 1.5 »).
        /// Vide : affiche le réglage actuel.
        amount: Option<String>,
    },

    /// Des idées de ce que tu peux faire avec Sparrow, classées par profil.
    /// « sparrow idees » · « sparrow idees enseignant » · « sparrow idees
    /// "factures" ».
    #[command(visible_aliases = ["ideas", "idées"])]
    Idees {
        /// Filtre : un profil (enseignant, developpeur, …) ou un mot-clé.
        filter: Vec<String>,
    },

    /// C'est quoi ce mot ? — définition instantanée d'un terme de Sparrow,
    /// en deux phrases simples, sans appel modèle. « sparrow whatis token ».
    #[command(name = "whatis", visible_aliases = ["c-est-quoi", "cest-quoi", "glossaire"])]
    Whatis {
        /// Le terme à définir (ex. checkpoint, token, swarm). Vide : liste les
        /// mots connus.
        term: Vec<String>,
    },

    /// Choisir comment Sparrow te parle : simple (langage clair), builder
    /// (workflows build), pro (sortie technique complète) ou auto. Sans
    /// argument : affiche le mode actuel.
    Mode {
        /// « simple », « builder », « pro » ou « auto ».
        mode: Option<String>,
    },

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

    /// Configure intelligent auto-routing provider
    Route {
        #[command(subcommand)]
        action: RouteAction,
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

    /// Manage local Sparrow plugins
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },

    /// Inspect and gate toolsets
    Tools {
        #[command(subcommand)]
        action: ToolsAction,
    },

    /// Security audit of config, permissions, plugins, hooks, secrets
    Security {
        #[command(subcommand)]
        action: SecurityAction,
    },

    /// GitHub Action / remote PR workflow
    Github {
        #[command(subcommand)]
        action: GithubAction,
    },

    /// Compact context and write a durable handoff doc
    Compact {
        /// Task description (recorded in the handoff)
        #[arg(long)]
        task: Option<String>,
        /// Output path (default: .sparrow/handoff/<timestamp>.md)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Emit JSON instead of Markdown to stdout (the file is always Markdown)
        #[arg(long)]
        json: bool,
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

    /// Manage saved sessions
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
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

    /// Inspect and update permission policy
    Permissions {
        #[command(subcommand)]
        action: PermissionAction,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Import config from another tool (claude-code, codex, opencode, openclaw)
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

    /// Run a self-contained demo (snake game)
    Demo,

    /// Share latest session as GitHub Gist
    Share,

    /// Install or scan security pre-commit hooks
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },

    /// Voice commands (speak, transcribe, providers)
    Voice {
        #[command(subcommand)]
        action: VoiceAction,
    },

    /// Test browser/vision (screenshot, navigate)
    Browser {
        /// URL to test
        #[arg(default_value = "https://example.com")]
        url: String,
    },
}

#[derive(Subcommand)]
pub enum AgentAction {
    Create { name: String },
    List,
    Edit { name: String },
    Rm { name: String },
    Run { name: String, task: String },
    Mention { name: String, message: String },
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
    View {
        name: String,
    },
    Create {
        name: String,
    },
    /// Install a skill from GitHub (gh:user/repo[/path]), a git URL, or a
    /// local path to a SKILL.md
    Install {
        source: String,
    },
    Update {
        name: String,
    },
    Prune,
    /// Remove a skill by name (e.g. to delete junk auto-learned skills)
    Rm {
        name: String,
    },
}

#[derive(Subcommand)]
pub enum PluginsAction {
    List,
    Install {
        source: String,
        #[arg(long)]
        allow: bool,
    },
    Rm {
        name: String,
    },
}

#[derive(Subcommand)]
pub enum GithubAction {
    /// Review a pull request: fetch diff via `gh`, run a read-only review prompt
    Review {
        /// PR number
        pr: u64,
        /// Print the review plan without invoking the model or posting comments
        #[arg(long)]
        dry_run: bool,
        /// Override the model id
        #[arg(long)]
        model: Option<String>,
        /// Restrict tool allow-list (comma-separated). Empty = inherit config.
        #[arg(long)]
        allowed_tools: Option<String>,
    },
    /// Show CI status for the current branch (via `gh run list`)
    Status,
    /// Fetch CI logs for a workflow run id (via `gh run view --log`)
    Logs { run_id: String },
}

#[derive(Subcommand)]
pub enum ReleaseAction {
    /// Prepare launch notes and migration notes from local artifacts.
    Prep {
        /// Show the target files without writing them.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum IntelAction {
    /// Fetch configured or explicit public sources into the local intel cache.
    Scan {
        /// TOML file containing [[source]] entries.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Explicit source as kind:name:url, e.g.
        /// github_releases:Codex:https://github.com/openai/codex
        #[arg(long)]
        source: Vec<String>,

        /// Max releases per GitHub source.
        #[arg(long, default_value_t = 5)]
        limit: usize,

        /// Emit JSON instead of a human summary.
        #[arg(long)]
        json: bool,
    },

    /// Show cached release digests without network access.
    Report {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },

    /// Show cached scored backlog tickets without network access.
    Backlog {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },

    /// Repeated opt-in scan loop. Requires intel.enabled=true or explicit sources.
    Watch {
        #[arg(long, default_value_t = 3600)]
        interval: u64,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        source: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum SecurityAction {
    /// Run a full security audit
    Audit {
        /// Emit JSON instead of human-readable summary
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ToolsAction {
    List {
        #[arg(long)]
        surface: Option<String>,
    },
    Enable {
        tool: String,
    },
    Disable {
        tool: String,
    },
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
    Health,
    Abort { run: String },
    Stop,
}

#[derive(Subcommand)]
pub enum SessionAction {
    List,
    Export {
        id: String,
        path: Option<PathBuf>,
    },
    Cleanup {
        #[arg(long, default_value_t = 30)]
        older_than_days: u64,
    },
    /// Full-text search across sessions
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum ProfileAction {
    Create { name: String },
    List,
    Use { name: String },
}

#[derive(Subcommand)]
pub enum ImportSource {
    /// Import from Claude Code (~/.claude/)
    ClaudeCode {
        /// Path to project with .claude/ directory (defaults to cwd)
        path: Option<PathBuf>,
    },
    /// Import from OpenAI Codex CLI (~/.codex/)
    Codex {
        /// Path to project with codex config (defaults to cwd)
        path: Option<PathBuf>,
    },
    /// Import from OpenCode (~/.config/opencode/)
    #[command(name = "opencode")]
    OpenCode {
        /// Path to project with opencode.json (defaults to cwd)
        path: Option<PathBuf>,
    },
    /// Import from OpenClaw (~/.openclaw/)
    Openclaw {
        /// Path to the OpenClaw config directory (defaults to ~/.openclaw)
        path: Option<PathBuf>,
    },
    /// Auto-detect installed tools and import each one
    Auto,
}

#[derive(Subcommand)]
pub enum MemoryAction {
    List,
    Forget {
        id: String,
    },
    Add {
        key: String,
        value: String,
    },
    Replace {
        id: String,
        key: String,
        value: String,
    },
    Recall {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Consolidate,
    Docs,
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Scroll {
        session: String,
        #[arg(long, default_value_t = 0)]
        around: usize,
        #[arg(long, default_value_t = 3)]
        before: usize,
        #[arg(long, default_value_t = 3)]
        after: usize,
    },
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
}

#[derive(Subcommand)]
pub enum GraphAction {
    UpsertNode {
        id: String,
        label: String,
        #[arg(long, default_value = "entity")]
        kind: String,
        #[arg(long, default_value = "{}")]
        properties: String,
    },
    UpsertEdge {
        from_id: String,
        relation: String,
        to_id: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        weight: f64,
        #[arg(long, default_value = "{}")]
        properties: String,
    },
    Get {
        id: String,
    },
    Neighbors {
        id: String,
        #[arg(long, default_value = "both")]
        direction: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Export,
    DeleteNode {
        id: String,
    },
    DeleteEdge {
        id: String,
    },
    SyncNeo4j,
}

#[derive(Subcommand)]
pub enum PermissionAction {
    /// Show current permission mode and rules
    List,
    /// Set permission mode (read-only|plan|supervised|trusted|autonomous|emergency-stop)
    Set { mode: String },
    /// Add an explicitly allowed tool pattern
    AllowTool { tool: String },
    /// Add a tool pattern that always asks for approval
    AskTool { tool: String },
    /// Add an explicitly denied tool pattern
    DenyTool { tool: String },
    /// Add an allowed path boundary
    AllowPath { path: PathBuf },
    /// Add a denied path boundary
    DenyPath { path: PathBuf },
}

#[derive(Subcommand)]
pub enum RouteAction {
    /// Pin routing to a specific provider or provider/model.
    /// Examples: sparrow route set deepseek | sparrow route set deepseek/deepseek-v4-pro
    Set {
        /// Provider ID, or provider/model (e.g. \"deepseek/deepseek-v4-pro\")
        provider: String,
    },
    /// Clear the pinned provider/model — let the multi-tier policy decide per task.
    Clear,
    /// Show the current routing config (preferred provider + per-tier policy).
    Show,
    /// Switch to manual mode — always use the chosen provider/model, never fall back.
    Manual,
    /// Switch to auto mode — tier-based policy + free_first fallback (default).
    Auto,
}

#[derive(Subcommand)]
pub enum HookAction {
    /// Install pre-commit security hook
    Install,
    /// Scan staged files (or all files with --all) for secrets
    Scan {
        /// Scan entire working tree instead of just staged files
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
pub enum VoiceAction {
    /// Convert text to speech
    Speak { text: String },
    /// Transcribe audio file
    Transcribe { file: String },
    /// List available voice providers
    Providers,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // v0.9 Pilier 1: the human front-door commands must collect their
    // free-text argument WITHOUT swallowing global flags like --yes. A first
    // implementation used `trailing_var_arg` and captured "--yes" into the
    // problem text — the model then complained about the stray flags.
    #[test]
    fn explique_does_not_swallow_global_flags() {
        let cli = Cli::parse_from(["sparrow", "explique", "borrow checker", "--yes"]);
        assert!(cli.yes, "--yes must be parsed as a flag, not text");
        match cli.command {
            Some(Commands::Explique { target }) => {
                assert_eq!(target, vec!["borrow checker".to_string()]);
            }
            _ => panic!("expected Explique"),
        }
    }

    #[test]
    fn fix_collects_words_and_respects_flags() {
        let cli = Cli::parse_from(["sparrow", "fix", "le", "build", "casse", "--yes"]);
        assert!(cli.yes);
        match cli.command {
            Some(Commands::Fix { problem }) => {
                assert_eq!(problem, vec!["le", "build", "casse"]);
            }
            _ => panic!("expected Fix"),
        }
    }

    #[test]
    fn fix_accepts_no_argument() {
        let cli = Cli::parse_from(["sparrow", "fix"]);
        match cli.command {
            Some(Commands::Fix { problem }) => assert!(problem.is_empty()),
            _ => panic!("expected Fix"),
        }
    }

    #[test]
    fn human_aliases_resolve() {
        // repare → Fix, explain → Explique, montre → Console, undo → Annule.
        assert!(matches!(
            Cli::parse_from(["sparrow", "repare", "x"]).command,
            Some(Commands::Fix { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "explain", "x"]).command,
            Some(Commands::Explique { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "montre"]).command,
            Some(Commands::Console { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "undo"]).command,
            Some(Commands::Annule { .. })
        ));
    }

    #[test]
    fn console_fast_flag_parses() {
        match Cli::parse_from(["sparrow", "console", "--fast"]).command {
            Some(Commands::Console { port, fast }) => {
                assert_eq!(port, 9339);
                assert!(fast);
            }
            _ => panic!("expected Console"),
        }
    }

    #[test]
    fn v092_audit_and_test_commands_parse() {
        assert!(matches!(
            Cli::parse_from(["sparrow", "audit", "--json"]).command,
            Some(Commands::Audit { json: true })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "test", "--fix"]).command,
            Some(Commands::Test {
                fix: true,
                json: false
            })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "commit", "--dry-run", "-m", "feat: x"]).command,
            Some(Commands::Commit {
                dry_run: true,
                message: Some(_)
            })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "release", "prep"]).command,
            Some(Commands::Release {
                action: ReleaseAction::Prep { dry_run: false }
            })
        ));
        assert!(matches!(
            Cli::parse_from([
                "sparrow",
                "intel",
                "scan",
                "--source",
                "github_releases:Codex:https://github.com/openai/codex",
                "--limit",
                "2"
            ])
            .command,
            Some(Commands::Intel {
                action: IntelAction::Scan { limit: 2, .. }
            })
        ));
        assert!(matches!(
            Cli::parse_from([
                "sparrow",
                "run",
                "fix it",
                "--plan-first",
                "--dry-run",
                "--patch"
            ])
            .command,
            Some(Commands::Run {
                plan_first: true,
                dry_run: true,
                patch: true,
                ..
            })
        ));
    }

    #[test]
    fn v09_human_commands_parse() {
        assert!(matches!(
            Cli::parse_from(["sparrow", "idees", "enseignant"]).command,
            Some(Commands::Idees { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "ideas"]).command,
            Some(Commands::Idees { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "whatis", "token"]).command,
            Some(Commands::Whatis { .. })
        ));
        assert!(matches!(
            Cli::parse_from(["sparrow", "c-est-quoi", "checkpoint"]).command,
            Some(Commands::Whatis { .. })
        ));
        match Cli::parse_from(["sparrow", "budget", "2€"]).command {
            Some(Commands::Budget { amount }) => assert_eq!(amount.as_deref(), Some("2€")),
            _ => panic!("expected Budget"),
        }
    }

    #[test]
    fn mode_command_parses_optional_argument() {
        match Cli::parse_from(["sparrow", "mode"]).command {
            Some(Commands::Mode { mode }) => assert!(mode.is_none()),
            _ => panic!("expected Mode"),
        }
        match Cli::parse_from(["sparrow", "mode", "pro"]).command {
            Some(Commands::Mode { mode }) => assert_eq!(mode.as_deref(), Some("pro")),
            _ => panic!("expected Mode"),
        }
        match Cli::parse_from(["sparrow", "mode", "builder"]).command {
            Some(Commands::Mode { mode }) => assert_eq!(mode.as_deref(), Some("builder")),
            _ => panic!("expected Mode"),
        }
    }

    #[test]
    fn annule_defaults_and_flags() {
        // No id → latest (None); --tout → whole session.
        match Cli::parse_from(["sparrow", "annule"]).command {
            Some(Commands::Annule { id, tout }) => {
                assert!(id.is_none());
                assert!(!tout);
            }
            _ => panic!("expected Annule"),
        }
        match Cli::parse_from(["sparrow", "annule", "--tout"]).command {
            Some(Commands::Annule { id, tout }) => {
                assert!(id.is_none());
                assert!(tout);
            }
            _ => panic!("expected Annule"),
        }
        match Cli::parse_from(["sparrow", "annule", "cp-123"]).command {
            Some(Commands::Annule { id, .. }) => assert_eq!(id.as_deref(), Some("cp-123")),
            _ => panic!("expected Annule"),
        }
    }
}
