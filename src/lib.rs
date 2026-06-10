#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::derivable_impls,
    clippy::format_in_format_args,
    clippy::if_same_then_else,
    clippy::iter_cloned_collect,
    clippy::manual_clamp,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::manual_pattern_char_comparison,
    clippy::needless_borrow,
    clippy::needless_range_loop,
    clippy::new_without_default,
    clippy::ptr_arg,
    clippy::should_implement_trait,
    clippy::single_match,
    clippy::type_complexity,
    clippy::unnecessary_cast,
    clippy::let_and_return,
    clippy::useless_conversion,
    clippy::useless_format,
    clippy::while_let_loop
)]

// The cmd_handlers/ modules were extracted from main.rs (a binary) and
// reference this crate by its public name `sparrow::…`. From inside the
// lib crate that path would normally fail — this alias makes it work.
extern crate self as sparrow;

pub mod agent;
pub mod auth;
pub mod autonomy;
pub mod capabilities;
pub mod chat;
pub mod cli;
pub mod cmd_handlers;
pub mod commands;
pub mod completions;
pub mod config;
pub mod console;
pub mod context;
pub mod cost;
pub mod demo;
pub mod engine;
pub mod errors;
pub mod event;
pub mod extras;
pub mod gateway;
pub mod github;
pub mod hook_cmd;
pub mod hooks;
pub mod instructions;
pub mod memory;
pub mod onboarding;
pub mod orchestrator;
pub mod permissions;
pub mod plan;
pub mod provider;
pub mod reasoning;
pub mod redaction;
pub mod router;
pub mod runtime;
pub mod sandbox;
pub mod security;
pub mod share;
pub mod streaming;
pub mod telemetry;
pub mod tools;
pub mod tui;
pub mod update;
