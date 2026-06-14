// src/cmd_handlers/handle_do_cmd.rs
//
// The natural-language front door. The PRIMARY path is bare text:
// `sparrow corrige le build` (no command word) is intercepted as an unknown
// subcommand and routed here. `sparrow do "<text>"` is the same thing, explicit.
//
// Resolution: a zero-cost FR/EN heuristic first; for phrasings it can't place,
// the routed model classifies (nl_router::route) — so Sparrow detects intent
// from context like a person would, not from memorized commands. The resolved
// command is announced, then executed (Safe/Reversible) or proposed (Confirm).
use super::prelude::*;
use sparrow::nl_router::{CommandIntent, CommandRisk, heuristic_match, risk_of, route};

fn risk_label(risk: CommandRisk) -> &'static str {
    match risk {
        CommandRisk::Safe => "sûr",
        CommandRisk::Reversible => "réversible",
        CommandRisk::Confirm => "à confirmer",
    }
}

/// Re-invoke this binary with the resolved subcommand — reuses every existing
/// handler. Safe: catalog commands are fixed strings (none is `do`/bare text, so
/// no recursion) and the payload is passed as a single argument.
fn execute_resolved(intent: &CommandIntent) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut args: Vec<String> = intent
        .command
        .split_whitespace()
        .map(String::from)
        .collect();
    if !intent.payload.is_empty() {
        args.push(intent.payload.clone());
    }
    let status = std::process::Command::new(exe).args(&args).status()?;
    if !status.success() {
        anyhow::bail!(
            "`{}` exited with {}",
            intent.as_invocation(),
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

/// Resolve free text to a command and act on it. Heuristic first; the model
/// classifier handles whatever the heuristic can't place. `dry_run` (or the
/// `SPARROW_NL_PREVIEW` env var) prints the resolution without executing.
pub async fn dispatch_natural_language(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    text: &str,
    dry_run: bool,
) -> anyhow::Result<()> {
    let text = text.trim();
    if text.is_empty() {
        anyhow::bail!(
            "Dis-moi ce que tu veux, en langage naturel. Ex : « sparrow corrige le build »."
        );
    }
    let preview = dry_run || std::env::var("SPARROW_NL_PREVIEW").is_ok();

    let intent = match heuristic_match(text) {
        Some(i) => i,
        None => {
            // Fuzzy phrasing → let the routed model classify against the catalog.
            let providers = build_provider_brains(config, &memory, false);
            let router = sparrow::router::BasicRouter::new(config, providers);
            let need = sparrow::router::RoutingNeed {
                tier: sparrow::router::TaskTier::Small,
                required_tools: false,
                required_vision: false,
                prefer_local: true,
            };
            let budget = sparrow::router::BudgetState {
                daily_limit_usd: config.budget.daily_usd,
                daily_spent_usd: 0.0,
                session_limit_usd: config.budget.session_usd,
                session_spent_usd: 0.0,
            };
            use sparrow::router::Router;
            match router.select(&need, &budget).first() {
                Some(brain) => route(brain.as_ref(), text).await,
                // No model available → hand the whole thing to the agent.
                None => CommandIntent {
                    command: "run".into(),
                    payload: text.to_string(),
                    confidence: 0.3,
                },
            }
        }
    };

    let risk = risk_of(&intent.command);
    println!(
        "🐦 → {}   ({:.0}% · {})",
        intent.as_invocation(),
        intent.confidence * 100.0,
        risk_label(risk)
    );

    if preview {
        return Ok(());
    }

    match risk {
        // Read-only or checkpointed/reversible → the user asked for it, do it.
        CommandRisk::Safe | CommandRisk::Reversible => execute_resolved(&intent),
        // Outward / hard-to-undo → don't auto-run; show exactly how to.
        CommandRisk::Confirm => {
            println!(
                "Action sensible — pour la lancer toi-même : {}",
                intent.as_invocation()
            );
            Ok(())
        }
    }
}

/// `sparrow do "<text>"` — the explicit form of the bare-text front door.
pub async fn handle_do(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    request: &[String],
    dry_run: bool,
) -> anyhow::Result<()> {
    dispatch_natural_language(config, memory, &request.join(" "), dry_run).await
}
