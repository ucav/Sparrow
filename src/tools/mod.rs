//! Tools layer. The tool registry (`Tool` trait, `Registry`, `ToolResult`,
//! `ToolCtx`) and most tool implementations (fs, edit, search, web, browser,
//! git, exec, todo, media, code-nav, the memory tool…) live in the
//! `sparrow-tools` crate so the heavy tool code compiles once and is cached.
//! Re-exported here so existing `crate::tools::*` imports keep working.
//!
//! `extras` and `subagent` stay in the binary crate: they sit at the top of the
//! dependency graph (they hold an `Arc<Engine>`, spawn sub-agents via the
//! `Engine`/`Router`, and carry `gateway::GatewayResponse`), which would create
//! cycles if pulled below the engine.

pub use sparrow_tools::*;

pub mod extras;
pub mod subagent;
