// src/cmd_handlers/handle_do_cmd.rs
//
// `sparrow do "<natural language>"` — the no-commands-to-learn front door.
// Resolves free text to a Sparrow command via the NL router (zero-cost
// heuristic; the model classifier is available via nl_router::route for the
// full path), shows the chosen command, then runs safe ones and proposes
// reversible/confirm ones.
use sparrow::nl_router::{CommandIntent, CommandRisk, heuristic_match, risk_of};

fn risk_label(risk: CommandRisk) -> &'static str {
    match risk {
        CommandRisk::Safe => "sûr",
        CommandRisk::Reversible => "réversible",
        CommandRisk::Confirm => "à confirmer",
    }
}

/// Re-invoke this binary with the resolved subcommand. Safe because the catalog
/// commands are fixed strings (no `do`, so no recursion) and the payload is a
/// single argument.
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

pub fn handle_do(request: &[String], dry_run: bool) -> anyhow::Result<()> {
    let text = request.join(" ");
    let text = text.trim();
    if text.is_empty() {
        anyhow::bail!("usage: sparrow do \"<ce que tu veux, en langage naturel>\"");
    }

    // Zero-cost heuristic; fall back to handing the whole thing to the agent.
    let intent = heuristic_match(text).unwrap_or_else(|| CommandIntent {
        command: "run".into(),
        payload: text.to_string(),
        confidence: 0.3,
    });
    let risk = risk_of(&intent.command);

    println!(
        "🐦 → {}   ({:.0}% · {})",
        intent.as_invocation(),
        intent.confidence * 100.0,
        risk_label(risk)
    );

    if dry_run {
        return Ok(());
    }

    match risk {
        // Read-only / safe → just do it.
        CommandRisk::Safe => execute_resolved(&intent),
        // Mutating or hard-to-undo → don't auto-run; show how to.
        CommandRisk::Reversible | CommandRisk::Confirm => {
            println!(
                "Commande modifiante — pour la lancer : {}",
                intent.as_invocation()
            );
            Ok(())
        }
    }
}
