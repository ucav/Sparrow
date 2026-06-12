//! Regression: `--max-wall-secs`, `--max-tokens`, `--max-cost-usd` were parsed
//! by clap but never applied to the run config (like `--bind` before v0.8.1),
//! so the engine never enforced them. These tests pin that the CLI flags now
//! flow into `config.budget`.

use clap::Parser;
use sparrow::cli::Cli;
use sparrow::cmd_handlers::handle_agent_cmd::apply_cli_overrides;
use sparrow::config::Config;

#[test]
fn cli_budget_caps_reach_the_config() {
    let cli = Cli::parse_from([
        "sparrow",
        "run",
        "do a thing",
        "--max-wall-secs",
        "30",
        "--max-tokens",
        "500",
        "--max-cost-usd",
        "0.25",
    ]);
    let mut config = Config::default();
    apply_cli_overrides(&mut config, &cli);

    assert_eq!(
        config.budget.max_wall_secs,
        Some(30),
        "--max-wall-secs ignored"
    );
    assert_eq!(config.budget.max_tokens, Some(500), "--max-tokens ignored");
    assert!(
        (config.budget.session_usd - 0.25).abs() < f64::EPSILON,
        "--max-cost-usd ignored"
    );
}

#[test]
fn no_caps_means_no_time_or_token_limit() {
    let cli = Cli::parse_from(["sparrow", "run", "x"]);
    let mut config = Config::default();
    apply_cli_overrides(&mut config, &cli);
    assert_eq!(config.budget.max_wall_secs, None);
    assert_eq!(config.budget.max_tokens, None);
}

#[test]
fn no_checkpoint_flag_disables_checkpointing() {
    let cli = Cli::parse_from(["sparrow", "run", "x", "--no-checkpoint"]);
    let mut config = Config::default();
    assert!(config.defaults.checkpointing, "default is checkpoints on");
    apply_cli_overrides(&mut config, &cli);
    assert!(
        !config.defaults.checkpointing,
        "--no-checkpoint must turn checkpointing off"
    );
}

#[test]
fn max_cost_usd_takes_precedence_over_budget() {
    // Both given: the more specific --max-cost-usd wins.
    let cli = Cli::parse_from([
        "sparrow",
        "run",
        "x",
        "--budget",
        "1.0",
        "--max-cost-usd",
        "0.10",
    ]);
    let mut config = Config::default();
    apply_cli_overrides(&mut config, &cli);
    assert!((config.budget.session_usd - 0.10).abs() < f64::EPSILON);
}
