//! v0.9 Pilier 2 — « zéro jargon ».
//!
//! One table, used by every surface (CLI, TUI, console, gateways), that turns
//! each engine [`Event`] into a plain-language status line for **simple mode**.
//! The transcript should read like a sentence — *tu as dit · je fais · voilà* —
//! never `run a3f2 · route … · tier T1`.
//!
//! ## The anti-regression lock
//! [`humanize`] matches **every** `Event` variant with no wildcard arm. Adding
//! a new variant to the contract therefore fails to compile here until someone
//! gives it a human phrase (or an explicit `None`). That is stronger than a
//! test: you cannot ship an un-humanized event.
//!
//! Events that are pure telemetry or internal continuity (token counts, cost
//! deltas, opaque reasoning) return `None` — in simple mode they belong to the
//! HUD, not the conversation.

use sparrow_core::event::{AgentStatus, AutonomyLevel, Decision, Event, RiskLevel};

/// Display language for the human layer. Only two are shipped on purpose
/// (the structure allows more); see PLAN_v0.9.0 §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    /// Resolve from a config/locale string. Anything not clearly English
    /// falls back to French (Sparrow's primary language today).
    pub fn from_code(code: &str) -> Self {
        let c = code.trim().to_lowercase();
        if c.starts_with("en") {
            Lang::En
        } else {
            Lang::Fr
        }
    }
}

/// Pick the language string. Tiny helper to keep the big match readable.
fn t(lang: Lang, fr: &str, en: &str) -> String {
    match lang {
        Lang::Fr => fr.to_string(),
        Lang::En => en.to_string(),
    }
}

/// Short, human verb phrase for a tool the agent is about to use.
/// Falls back to a generic phrasing for unknown tools.
fn tool_phrase(name: &str, args: &serde_json::Value, lang: Lang) -> String {
    // The file/path argument under any of the common keys.
    let target = ["path", "file_path", "file", "filename"]
        .iter()
        .find_map(|k| args.get(*k).and_then(|v| v.as_str()))
        .unwrap_or("");
    let with = |fr: &str, en: &str| {
        if target.is_empty() {
            t(lang, fr, en)
        } else {
            match lang {
                Lang::Fr => format!("{fr} {target}…"),
                Lang::En => format!("{en} {target}…"),
            }
        }
    };
    match name {
        "fs_read" | "read" | "read_file" | "file_search" => with("Je lis", "Reading"),
        "fs_write" | "write" | "write_file" => with("Je crée", "Creating"),
        "edit" | "multi_edit" | "str_replace" => with("Je modifie", "Editing"),
        "exec" | "bash" | "shell" | "code_exec" | "run_command" => {
            t(lang, "Je lance une commande…", "Running a command…")
        }
        "search" | "grep" | "ripgrep" => t(lang, "Je cherche dans le code…", "Searching the code…"),
        "web_search" => t(lang, "Je cherche sur internet…", "Searching the web…"),
        "web_fetch" | "fetch" => t(lang, "Je consulte une page web…", "Fetching a web page…"),
        _ => match lang {
            Lang::Fr => format!("Je m'apprête à utiliser l'outil « {name} »…"),
            Lang::En => format!("About to use the “{name}” tool…"),
        },
    }
}

/// Translate an engine event into a plain-language status line for simple mode.
///
/// Returns `None` for events that should not appear as a status line in the
/// conversation (telemetry, internal continuity, or content that the renderer
/// already prints verbatim, like streamed assistant text).
pub fn humanize(ev: &Event, lang: Lang) -> Option<String> {
    match ev {
        Event::RunStarted { .. } => Some(t(lang, "C'est parti, je m'occupe de ça.", "On it.")),

        // Routing/continuity details belong to the HUD, not the conversation.
        Event::RouteSelected { .. } => None,
        Event::TokenUsage { .. } => None,
        Event::TokenUsageEstimated { .. } => None,
        Event::CostUpdate { .. } => None,
        // Streamed assistant text and opaque reasoning are rendered (or hidden)
        // by the normal text path — not as a status line.
        Event::ThinkingDelta { .. } => None,
        Event::ReasoningDelta { .. } => None,
        // Per-agent lane chatter shows in the cockpit lanes, not the simple feed.
        Event::AgentStatus { .. } => None,
        // Skill bookkeeping is invisible in simple mode.
        Event::SkillLearned { .. } => None,

        Event::ModelSwitched { reason, .. } => {
            // Honest, non-technical framing of an escalation/fallback.
            if reason.contains("escalat") || reason.contains("verify") {
                Some(t(
                    lang,
                    "C'est plus coriace que prévu, je passe la vitesse supérieure.",
                    "Tougher than expected — stepping up to a stronger model.",
                ))
            } else {
                Some(t(
                    lang,
                    "Je change de modèle pour continuer.",
                    "Switching model to keep going.",
                ))
            }
        }

        // Conversation content. The router line is hidden in simple mode and
        // other roles are printed verbatim by the renderer, so the humanize
        // table emits no status line for any Message.
        Event::Message { .. } => None,

        Event::ToolUseProposed { name, args, .. } => Some(tool_phrase(name, args, lang)),
        // The "started" echo would duplicate the proposed line.
        Event::ToolUseStarted { .. } => None,
        Event::ToolOutput { .. } => None,

        Event::ApprovalRequested { tool, .. } => {
            let what = tool.as_deref().unwrap_or("");
            if what.is_empty() {
                Some(t(
                    lang,
                    "J'ai besoin de ton accord pour continuer.",
                    "I need your go-ahead to continue.",
                ))
            } else {
                Some(match lang {
                    Lang::Fr => format!("J'ai besoin de ton accord pour « {what} »."),
                    Lang::En => format!("I need your go-ahead for “{what}”."),
                })
            }
        }
        Event::ApprovalResolved { decision, .. } => Some(match decision {
            Decision::Allow
            | Decision::AllowOnce
            | Decision::AllowSession
            | Decision::AllowAlways => t(lang, "D'accord, j'y vais.", "Got it, going ahead."),
            Decision::Deny => t(
                lang,
                "Compris, je n'y touche pas.",
                "Understood, leaving it alone.",
            ),
            Decision::AskUser => t(lang, "J'attends ta réponse.", "Waiting for your answer."),
        }),

        Event::DiffProposed {
            file, plus, minus, ..
        } => Some(match lang {
            Lang::Fr => format!("J'ai préparé une modification de {file} (+{plus} / −{minus})."),
            Lang::En => format!("Prepared a change to {file} (+{plus} / −{minus})."),
        }),
        Event::DiffApplied { file, .. } => Some(match lang {
            Lang::Fr => format!("{file} mis à jour."),
            Lang::En => format!("{file} updated."),
        }),

        Event::TestResult { passed, failed, .. } => Some(if *failed == 0 {
            match lang {
                Lang::Fr => format!("Tests : {passed} réussis. ✅"),
                Lang::En => format!("Tests: {passed} passing. ✅"),
            }
        } else {
            match lang {
                Lang::Fr => format!("Tests : {passed} réussis, {failed} en échec."),
                Lang::En => format!("Tests: {passed} passing, {failed} failing."),
            }
        }),

        Event::AgentSpawned { role, .. } => Some(match lang {
            Lang::Fr => format!("Je fais appel à un assistant ({role})."),
            Lang::En => format!("Bringing in a helper ({role})."),
        }),

        Event::CheckpointCreated { .. } => Some(t(
            lang,
            "Point de sauvegarde fait — on peut tout annuler.",
            "Checkpoint saved — everything is undoable.",
        )),

        Event::AutonomyChanged { level, .. } => Some(match level {
            AutonomyLevel::Supervised => t(
                lang,
                "Je te demande avant chaque action.",
                "I'll ask before each action.",
            ),
            AutonomyLevel::Trusted => t(
                lang,
                "J'agis seul, mais je te montre tout.",
                "I'll act on my own and show you everything.",
            ),
            AutonomyLevel::Autonomous => {
                t(lang, "Je travaille en autonomie.", "Working autonomously.")
            }
        }),

        Event::RunFinished { outcome, .. } => {
            let files = outcome.diffs.len();
            Some(match outcome.status.as_str() {
                "completed" => match lang {
                    Lang::Fr if files > 0 => {
                        format!("Terminé ! {files} fichier(s) modifié(s).")
                    }
                    Lang::Fr => "Terminé !".to_string(),
                    Lang::En if files > 0 => format!("Done! {files} file(s) changed."),
                    Lang::En => "Done!".to_string(),
                },
                "waiting_for_approval" => t(
                    lang,
                    "En attente de ton accord pour continuer.",
                    "Waiting for your approval to continue.",
                ),
                "no actions taken" => t(lang, "Rien n'a été modifié.", "Nothing was changed."),
                other => match lang {
                    Lang::Fr => format!("Fin : {other}."),
                    Lang::En => format!("Finished: {other}."),
                },
            })
        }

        Event::Error { message, .. } => {
            // Phase 2.3 — errors that reassure: never dump a raw API/JSON blob
            // in simple mode. Lead with a calm sentence and always offer an
            // exit door. Keep a short hint only when the message is itself
            // short and human.
            let m = message.to_lowercase();
            let calm = if m.contains("api error") || m.contains("400") || m.contains("{\"") {
                t(
                    lang,
                    "Un modèle a refusé la requête. Je réessaie autrement — si ça persiste, tape « sparrow doctor ».",
                    "A model refused the request. I'll try another way — if it persists, run “sparrow doctor”.",
                )
            } else if m.contains("connect") || m.contains("network") || m.contains("timeout") {
                t(
                    lang,
                    "Je n'arrive pas à joindre internet. Je peux continuer avec un modèle local si tu veux.",
                    "I can't reach the internet. I can keep going with a local model if you like.",
                )
            } else if message.len() <= 120 {
                match lang {
                    Lang::Fr => {
                        format!("Quelque chose a coincé : {message}. Rien n'a été modifié.")
                    }
                    Lang::En => format!("Something went wrong: {message}. Nothing was changed."),
                }
            } else {
                t(
                    lang,
                    "Quelque chose a coincé, mais rien n'a été modifié. Tape « sparrow doctor » pour un diagnostic.",
                    "Something went wrong, but nothing was changed. Run “sparrow doctor” for a checkup.",
                )
            };
            Some(calm)
        }

        Event::Compacted { .. } => Some(t(
            lang,
            "J'ai fait de la place dans ma mémoire de travail.",
            "Freed up room in my working memory.",
        )),

        Event::UpdateAvailable { latest, .. } => Some(match lang {
            Lang::Fr => format!("Une nouvelle version de Sparrow est disponible ({latest})."),
            Lang::En => format!("A new version of Sparrow is available ({latest})."),
        }),
    }
}

/// Plain-language label for a risk level (used in approval contracts).
pub fn risk_phrase(risk: &RiskLevel, lang: Lang) -> String {
    match risk {
        RiskLevel::ReadOnly => t(lang, "lecture seule", "read-only"),
        RiskLevel::Mutating => t(lang, "modifie des fichiers", "changes files"),
        RiskLevel::Exec => t(lang, "exécute une commande", "runs a command"),
        RiskLevel::Destructive => t(lang, "action irréversible", "irreversible action"),
        RiskLevel::Network => t(lang, "accède à internet", "accesses the internet"),
    }
}

/// Plain-language label for an agent status (used by the cockpit's simple view).
pub fn status_phrase(status: &AgentStatus, lang: Lang) -> String {
    match status {
        AgentStatus::Idle => t(lang, "au repos", "idle"),
        AgentStatus::Thinking => t(lang, "réfléchit…", "thinking…"),
        AgentStatus::Working => t(lang, "travaille…", "working…"),
        AgentStatus::WaitingForApproval => t(lang, "attend ton accord", "awaiting your go-ahead"),
        AgentStatus::Done => t(lang, "terminé", "done"),
        AgentStatus::Error => t(lang, "a rencontré un souci", "hit a problem"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sparrow_core::event::{OutcomeSummary, RunId, TokenUsage};
    use serde_json::json;

    fn run() -> RunId {
        RunId("test".into())
    }

    #[test]
    fn experience_config_resolves_mode_and_lang() {
        use crate::config::ExperienceConfig;
        let pro = ExperienceConfig {
            mode: "pro".into(),
            language: "en".into(),
        };
        assert!(!pro.is_simple());
        assert_eq!(pro.lang(), Lang::En);

        let auto = ExperienceConfig {
            mode: "auto".into(),
            language: "fr".into(),
        };
        assert!(auto.is_simple(), "auto resolves to simple (human-first)");
        assert_eq!(auto.lang(), Lang::Fr);

        let builder = ExperienceConfig {
            mode: "builder".into(),
            language: "fr".into(),
        };
        assert!(builder.is_builder());
        assert!(!builder.is_simple());

        // Default is auto/auto → simple.
        assert!(ExperienceConfig::default().is_simple());
    }

    #[test]
    fn lang_from_code_defaults_to_french() {
        assert_eq!(Lang::from_code("fr-FR"), Lang::Fr);
        assert_eq!(Lang::from_code("en_US"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::Fr); // unknown → primary lang
        assert_eq!(Lang::from_code(""), Lang::Fr);
    }

    #[test]
    fn run_started_is_human_in_both_languages() {
        let ev = Event::RunStarted {
            run: run(),
            task: "x".into(),
            agent: "sparrow".into(),
        };
        assert_eq!(
            humanize(&ev, Lang::Fr).unwrap(),
            "C'est parti, je m'occupe de ça."
        );
        assert_eq!(humanize(&ev, Lang::En).unwrap(), "On it.");
    }

    #[test]
    fn tool_proposed_names_the_file_in_plain_words() {
        let ev = Event::ToolUseProposed {
            run: run(),
            id: "1".into(),
            name: "fs_write".into(),
            args: json!({"path": "poeme.txt", "content": "x"}),
            risk: RiskLevel::Mutating,
        };
        assert_eq!(humanize(&ev, Lang::Fr).unwrap(), "Je crée poeme.txt…");
    }

    #[test]
    fn telemetry_events_have_no_status_line() {
        for ev in [
            Event::TokenUsage {
                run: run(),
                input: 10,
                output: 5,
            },
            Event::CostUpdate {
                run: run(),
                usd: 0.01,
            },
            Event::ReasoningDelta {
                run: run(),
                text: "…".into(),
            },
            Event::RouteSelected {
                run: run(),
                chain: vec![],
                context_window: 1,
            },
        ] {
            assert!(
                humanize(&ev, Lang::Fr).is_none(),
                "telemetry must be silent in simple mode"
            );
        }
    }

    #[test]
    fn no_jargon_leaks_into_simple_mode() {
        // A sweep of representative events must never surface raw identifiers
        // or technical tokens in their human phrasing.
        let banned = ["run ", "tier", "T1", "tok", "route ", "↑", "↓", "$0.0"];
        let samples = [
            Event::RunStarted {
                run: run(),
                task: "t".into(),
                agent: "sparrow".into(),
            },
            Event::CheckpointCreated {
                run: run(),
                id: sparrow_core::event::CheckpointId("c".into()),
                label: "l".into(),
            },
            Event::RunFinished {
                run: run(),
                outcome: OutcomeSummary {
                    status: "completed".into(),
                    diffs: vec![],
                    cost_usd: 0.0,
                    tokens: TokenUsage {
                        input: 0,
                        output: 0,
                    },
                    cost_comparison: String::new(),
                    duration_ms: None,
                },
            },
        ];
        for ev in samples {
            if let Some(phrase) = humanize(&ev, Lang::Fr) {
                for bad in banned {
                    assert!(
                        !phrase.contains(bad),
                        "simple-mode phrase `{phrase}` leaked jargon `{bad}`"
                    );
                }
            }
        }
    }

    #[test]
    fn error_is_reassuring_and_hides_raw_blobs() {
        let ev = Event::Error {
            run: run(),
            message: "OpenAI-compatible API error 400: {\"error\":{\"message\":\"…\"}}".into(),
        };
        let line = humanize(&ev, Lang::Fr).unwrap();
        assert!(!line.contains('{'), "raw JSON must not leak in simple mode");
        assert!(line.contains("doctor"), "must offer an exit door");
    }

    #[test]
    fn run_finished_reports_files_changed() {
        let ev = Event::RunFinished {
            run: run(),
            outcome: OutcomeSummary {
                status: "completed".into(),
                diffs: vec![sparrow_core::event::FileDiff {
                    file: "a.txt".into(),
                    plus: 1,
                    minus: 0,
                }],
                cost_usd: 0.0,
                tokens: TokenUsage {
                    input: 0,
                    output: 0,
                },
                cost_comparison: String::new(),
                duration_ms: None,
            },
        };
        assert_eq!(
            humanize(&ev, Lang::Fr).unwrap(),
            "Terminé ! 1 fichier(s) modifié(s)."
        );
    }
}
