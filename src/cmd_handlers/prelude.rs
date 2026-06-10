// src/cmd_handlers/prelude.rs — common imports for all handler modules.
//
// Every extracted handler does `use super::prelude::*;` and gets the full
// CLI vocabulary in one shot: types, traits, helpers, and the per-run
// flags. Anything a handler needs from elsewhere in the crate goes here so
// the handlers stay short.

pub use std::collections::HashMap;
pub use std::path::{Path, PathBuf};
pub use std::sync::Arc;

pub use anyhow::anyhow;
pub use sparrow::agent::{AgentStore, FsAgentStore, Soul};
pub use sparrow::auth::{AuthStore, Credential};
pub use sparrow::autonomy::GitCheckpoints;
pub use sparrow::capabilities::mcp::{BasicMcpClient, McpClient, McpServer, Transport};
pub use sparrow::capabilities::{FsSkillLibrary, Skill, SkillLibrary};
pub use sparrow::cli::{Cli, Commands, ImportSource};
pub use sparrow::config::{Config, ConfigStore, FsConfigStore, ProviderConfig};
pub use sparrow::console::WebViewServer;
pub use sparrow::engine::Engine;
pub use sparrow::event::{AutonomyLevel, Event, RunId};
pub use sparrow::extras::{ChatSession, Distiller, ReExecuter};
pub use sparrow::gateway::discord::DiscordTransport;
pub use sparrow::gateway::slack::SlackTransport;
pub use sparrow::gateway::telegram::TelegramTransport;
pub use sparrow::gateway::ws::WebSocketApi;
pub use sparrow::gateway::{GatewayMessage, GatewayResponse, GatewayTransport, MessageRouter};
pub use sparrow::memory::{Memory, SqliteMemory};
pub use sparrow::onboarding::migration::Migration;
pub use sparrow::runtime::event_bus::EventBus;
pub use sparrow::runtime::recorder::{FsRecorder, Recorder, Replayer, RunInputs};
pub use sparrow::runtime::scheduler::{Job, MemoryScheduler, Scheduler};
pub use sparrow::runtime::{Runtime, SparrowRuntime};
pub use sparrow::tui::Tui;

// Per-run flags. These live in the prelude (rather than in one handler) so
// every handler that calls `run_task` can construct them.

/// How `run_task` resolves the conversation context for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionMode {
    /// Continue this directory's session (existing behaviour).
    #[default]
    Auto,
    /// Ignore any saved session — start clean.
    Fresh,
    /// Continue the most recently updated session from ANY surface.
    ContinueLast,
}

/// Per-invocation run options threaded from the CLI flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunFlags {
    pub session_mode: SessionMode,
    /// Skip the pre-run quote confirmation (--yes, or non-interactive).
    pub assume_yes: bool,
}

// Cross-handler helpers. They live in run_task_cmd, agent_cmd, gateway_cmd,
// and memory_graph_cmd respectively — re-exported here so any handler can
// call them via the prelude without having to remember which sibling owns
// each one.

pub use super::handle_agent_cmd::{build_provider_brains, config_for_soul};
pub use super::handle_gateway_cmd::sanitize_file_component;
pub use super::handle_memory_graph_cmd::current_repo_head;
pub use super::handle_run_task_cmd::{redacted_config_snapshot, run_task};
