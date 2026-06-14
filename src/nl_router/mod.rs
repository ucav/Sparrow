//! Natural-language command router — "no commands to learn."
//!
//! Maps free-text input ("corrige le bug dans auth.rs", "show me the cockpit",
//! "what does this function do?") to a Sparrow [`CommandIntent`]. Two tiers:
//!
//! 1. [`heuristic_match`] — a zero-cost deterministic pass for the common,
//!    unambiguous phrasings (FR + EN). No model call.
//! 2. [`classifier_prompt`] + [`parse_intent`] — when the heuristic is unsure,
//!    ask the routed model to emit a JSON intent, validated against the catalog
//!    so it can never invent a command.
//!
//! The catalog ([`command_catalog`]) is the single source of truth: each entry
//! carries a natural-language description, example phrasings, whether it takes a
//! free-text payload, and a risk level the dispatcher uses to decide
//! execute-vs-confirm.

use crate::provider::Brain;
use crate::reasoning::inference_scaling::collect_text;

/// How risky a command is to run unattended — drives execute / confirm / suggest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    /// Read-only / safe — can run immediately.
    Safe,
    /// Mutates the workspace but is checkpointed/reversible — run, but announce.
    Reversible,
    /// Outward or hard-to-undo — confirm before running.
    Confirm,
}

/// One routable command.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// The canonical `sparrow` subcommand (what gets executed).
    pub command: &'static str,
    /// One-line natural-language description (fed to the classifier).
    pub description: &'static str,
    /// Whether the command takes a free-text payload (the user's words).
    pub takes_payload: bool,
    /// Example phrasings that should resolve here.
    pub examples: &'static [&'static str],
    /// Lowercased keyword/substring triggers for the heuristic pass.
    pub triggers: &'static [&'static str],
    pub risk: CommandRisk,
}

/// A resolved intent: which command to run, with what free-text payload.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandIntent {
    pub command: String,
    pub payload: String,
    /// 0.0–1.0. Heuristic hits are high; classifier supplies its own.
    pub confidence: f32,
}

impl CommandIntent {
    /// Render as the `sparrow` invocation it represents (for display/confirm).
    pub fn as_invocation(&self) -> String {
        if self.payload.is_empty() {
            format!("sparrow {}", self.command)
        } else {
            format!("sparrow {} \"{}\"", self.command, self.payload)
        }
    }
}

/// The routable command catalog. Representative, extensible — the dispatcher
/// maps `command` back to the real handler.
pub fn command_catalog() -> &'static [CommandSpec] {
    &[
        CommandSpec {
            command: "fix",
            description: "Diagnose and fix a problem, bug, crash, or failing build the user describes.",
            takes_payload: true,
            examples: &[
                "corrige le bug dans auth.rs",
                "my site crashes",
                "répare le build",
                "fix the failing test",
            ],
            triggers: &[
                "corrige",
                "répare",
                "repare",
                "fix",
                "plante",
                "crash",
                "bug",
                "erreur",
                "ça marche pas",
                "casse",
                "broken",
                "failing",
            ],
            risk: CommandRisk::Reversible,
        },
        CommandSpec {
            command: "explique",
            description: "Explain a file, error, concept, or piece of code in plain language.",
            takes_payload: true,
            examples: &[
                "explique src/app.js",
                "c'est quoi un borrow checker",
                "what does this function do",
                "comprends cette erreur",
            ],
            triggers: &[
                "explique",
                "explain",
                "c'est quoi",
                "qu'est-ce que",
                "comprends",
                "what is",
                "what does",
                "how does",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "plan",
            description: "Propose an approach or plan for a task, read-only, without making changes.",
            takes_payload: true,
            examples: &[
                "propose une approche pour ajouter l'auth",
                "how should I structure this",
                "plan the refactor",
            ],
            triggers: &[
                "propose",
                "plan",
                "approche",
                "comment faire",
                "how should",
                "strategy",
                "stratégie",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "run",
            description: "Carry out a general coding/agent task: implement, refactor, write, edit, run something.",
            takes_payload: true,
            examples: &[
                "ajoute la pagination à l'API",
                "write a TODO.md",
                "refactor this module",
                "lance les tests",
            ],
            triggers: &[
                "ajoute",
                "implémente",
                "implemente",
                "écris",
                "ecris",
                "refactor",
                "crée",
                "cree",
                "add",
                "implement",
                "write",
                "build me",
                "lance les tests",
                "run the tests",
            ],
            risk: CommandRisk::Reversible,
        },
        CommandSpec {
            command: "annule",
            description: "Undo the last change, roll back to the previous checkpoint.",
            takes_payload: false,
            examples: &["annule", "undo", "reviens en arrière", "rollback that"],
            triggers: &[
                "annule",
                "undo",
                "reviens en arrière",
                "rollback",
                "roll back",
                "défais",
                "defais",
            ],
            risk: CommandRisk::Confirm,
        },
        CommandSpec {
            command: "console",
            description: "Open the WebView cockpit / graphical interface.",
            takes_payload: false,
            examples: &["montre la console", "open the cockpit", "ouvre l'interface"],
            triggers: &[
                "console",
                "cockpit",
                "montre",
                "ouvre l'interface",
                "open the cockpit",
                "webview",
                "interface graphique",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "reason",
            description: "Answer a hard reasoning question with maximum rigor (inference-time scaling).",
            takes_payload: true,
            examples: &[
                "prouve que sqrt(2) est irrationnel",
                "reason carefully about this trade-off",
                "think hard about",
            ],
            triggers: &[
                "prouve",
                "raisonne",
                "reason carefully",
                "think hard",
                "démontre",
                "demontre",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "security audit",
            description: "Run a defensive security audit of the project.",
            takes_payload: false,
            examples: &[
                "audit de sécurité",
                "scan for vulnerabilities",
                "vérifie la sécurité",
            ],
            triggers: &[
                "sécurité",
                "securite",
                "security",
                "audit",
                "vulnérab",
                "vulnerab",
                "scan",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "model --list",
            description: "List the available providers and models.",
            takes_payload: false,
            examples: &[
                "quels modèles sont dispo",
                "list the models",
                "montre les providers",
            ],
            triggers: &["modèles", "modeles", "models", "providers", "provider"],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "memory list",
            description: "Show what Sparrow remembers (stored facts/preferences).",
            takes_payload: false,
            examples: &[
                "qu'est-ce que tu retiens",
                "what do you remember",
                "montre la mémoire",
            ],
            triggers: &[
                "mémoire",
                "memoire",
                "memory",
                "tu retiens",
                "remember",
                "souviens",
            ],
            risk: CommandRisk::Safe,
        },
        CommandSpec {
            command: "doctor",
            description: "Diagnose Sparrow's own setup/health.",
            takes_payload: false,
            examples: &[
                "est-ce que tout va bien",
                "check sparrow health",
                "diagnostic",
            ],
            triggers: &[
                "doctor",
                "diagnostic",
                "santé",
                "sante",
                "health",
                "tout va bien",
            ],
            risk: CommandRisk::Safe,
        },
    ]
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Strip a leading trigger phrase from `input` to recover the payload.
fn payload_after_trigger(input: &str, trigger: &str) -> String {
    let lower = input.to_lowercase();
    match lower.find(trigger) {
        Some(pos) => input[pos + trigger.len()..]
            .trim_start_matches([':', ' ', '\t'])
            .trim()
            .to_string(),
        None => input.trim().to_string(),
    }
}

/// Deterministic, zero-cost first pass. Returns a high-confidence intent when a
/// trigger matches unambiguously; `None` when it should defer to the model.
pub fn heuristic_match(input: &str) -> Option<CommandIntent> {
    let norm = normalize(input);
    if norm.is_empty() {
        return None;
    }
    // Longest trigger wins, so "run the tests" beats a bare "run"-like token.
    let mut best: Option<(&CommandSpec, &str)> = None;
    for spec in command_catalog() {
        for trig in spec.triggers {
            if norm.contains(trig) && best.map(|(_, t)| trig.len() > t.len()).unwrap_or(true) {
                best = Some((spec, trig));
            }
        }
    }
    let (spec, trig) = best?;
    let payload = if spec.takes_payload {
        payload_after_trigger(input, trig)
    } else {
        String::new()
    };
    Some(CommandIntent {
        command: spec.command.to_string(),
        payload,
        confidence: 0.85,
    })
}

/// Build the classifier prompt for the model path.
pub fn classifier_prompt(input: &str) -> String {
    let mut catalog = String::new();
    for spec in command_catalog() {
        catalog.push_str(&format!(
            "- {} — {} (payload: {})\n",
            spec.command,
            spec.description,
            if spec.takes_payload { "yes" } else { "no" }
        ));
    }
    format!(
        "You map a user's natural-language request to ONE Sparrow command from the catalog.\n\
         Reply with ONLY a JSON object: {{\"command\": \"<exact command from the catalog>\", \"payload\": \"<the user's task text, or empty>\", \"confidence\": <0.0-1.0>}}.\n\
         Use the user's own words for payload. If nothing fits, use command \"run\" with the full text as payload.\n\n\
         CATALOG:\n{catalog}\nUSER REQUEST:\n{input}\n\nJSON:"
    )
}

/// Parse the classifier's JSON reply into a validated intent. The command must
/// exist in the catalog (case-insensitive), else `None` — the model can never
/// invent a command.
pub fn parse_intent(reply: &str) -> Option<CommandIntent> {
    // Tolerate prose around the JSON: take the first {...} block.
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    let json = &reply[start..=end];
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let command = value.get("command")?.as_str()?.trim().to_string();
    let valid = command_catalog()
        .iter()
        .any(|s| s.command.eq_ignore_ascii_case(&command));
    if !valid {
        return None;
    }
    let payload = value
        .get("payload")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let confidence = value
        .get("confidence")
        .and_then(|c| c.as_f64())
        .map(|c| c as f32)
        .unwrap_or(0.6)
        .clamp(0.0, 1.0);
    Some(CommandIntent {
        command,
        payload,
        confidence,
    })
}

/// Resolve free text to an intent: heuristic first, then the model. Falls back
/// to a `run` intent with the raw text when the model path yields nothing, so
/// the user is never stuck — the agent just does the task.
pub async fn route(brain: &dyn Brain, input: &str) -> CommandIntent {
    if let Some(hit) = heuristic_match(input) {
        return hit;
    }
    let prompt = classifier_prompt(input);
    let req = crate::provider::BrainRequest {
        system: Some("You are a precise command router. Output only JSON.".into()),
        messages: vec![crate::provider::Msg {
            role: "user".into(),
            content: vec![crate::provider::ContentBlock::Text { text: prompt }],
        }],
        tools: vec![],
        max_tokens: 200,
        temperature: 0.0,
        stop: vec![],
        cache: crate::provider::PromptCacheConfig::disabled(),
    };
    if let Some(reply) = collect_text(brain, req).await {
        if let Some(intent) = parse_intent(&reply) {
            return intent;
        }
    }
    // Last resort: hand the whole thing to the general agent.
    CommandIntent {
        command: "run".into(),
        payload: input.trim().to_string(),
        confidence: 0.3,
    }
}

/// Look up the risk level for a resolved command.
pub fn risk_of(command: &str) -> CommandRisk {
    command_catalog()
        .iter()
        .find(|s| s.command.eq_ignore_ascii_case(command))
        .map(|s| s.risk)
        .unwrap_or(CommandRisk::Reversible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_routes_common_french_and_english() {
        assert_eq!(
            heuristic_match("corrige le bug dans auth.rs")
                .unwrap()
                .command,
            "fix"
        );
        assert_eq!(heuristic_match("my site crashes").unwrap().command, "fix");
        assert_eq!(
            heuristic_match("explique src/app.js").unwrap().command,
            "explique"
        );
        assert_eq!(heuristic_match("annule").unwrap().command, "annule");
        assert_eq!(
            heuristic_match("ouvre l'interface").unwrap().command,
            "console"
        );
        assert_eq!(
            heuristic_match("quels modèles sont dispo").unwrap().command,
            "model --list"
        );
        assert_eq!(
            heuristic_match("fais un audit de sécurité")
                .unwrap()
                .command,
            "security audit"
        );
    }

    #[test]
    fn heuristic_extracts_payload_after_trigger() {
        let intent = heuristic_match("explique le borrow checker").unwrap();
        assert_eq!(intent.command, "explique");
        assert_eq!(intent.payload, "le borrow checker");

        // Non-payload command keeps an empty payload.
        let undo = heuristic_match("undo").unwrap();
        assert!(undo.payload.is_empty());
    }

    #[test]
    fn heuristic_returns_none_for_unmatched() {
        assert!(heuristic_match("xyzzy quux").is_none());
        assert!(heuristic_match("").is_none());
    }

    #[test]
    fn parse_intent_validates_against_catalog() {
        let ok =
            parse_intent(r#"{"command":"fix","payload":"the build","confidence":0.9}"#).unwrap();
        assert_eq!(ok.command, "fix");
        assert_eq!(ok.payload, "the build");

        // Tolerates surrounding prose.
        let messy = parse_intent("Sure! {\"command\": \"plan\", \"payload\": \"x\"} done").unwrap();
        assert_eq!(messy.command, "plan");

        // Invented command → rejected.
        assert!(parse_intent(r#"{"command":"selfdestruct","payload":""}"#).is_none());
        assert!(parse_intent("not json").is_none());
    }

    #[test]
    fn invocation_rendering() {
        let i = CommandIntent {
            command: "fix".into(),
            payload: "le build".into(),
            confidence: 0.9,
        };
        assert_eq!(i.as_invocation(), "sparrow fix \"le build\"");
        let j = CommandIntent {
            command: "doctor".into(),
            payload: String::new(),
            confidence: 0.9,
        };
        assert_eq!(j.as_invocation(), "sparrow doctor");
    }

    #[test]
    fn risk_levels_resolve() {
        assert_eq!(risk_of("explique"), CommandRisk::Safe);
        assert_eq!(risk_of("annule"), CommandRisk::Confirm);
        assert_eq!(risk_of("fix"), CommandRisk::Reversible);
    }
}
