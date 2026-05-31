use crate::config::Config;

pub mod migration;
pub mod enterprise;

// ─── User mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UserMode {
    Beginner,
    Expert,
}

impl Default for UserMode {
    fn default() -> Self { UserMode::Beginner }
}

// ─── Onboarding engine ─────────────────────────────────────────────────────────

pub struct Onboarding {
    pub mode: UserMode,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    pub description: String,
    pub command: String,
    pub expected_output: String,
}

impl Default for Onboarding {
    fn default() -> Self {
        Self {
            mode: UserMode::Beginner,
            lessons: default_lessons(),
        }
    }
}

fn default_lessons() -> Vec<Lesson> {
    vec![
        Lesson {
            id: "run".into(),
            title: "Your First Run".into(),
            description: "Execute a simple task and see Sparrow in action.".into(),
            command: r#"run "Create a file called hello.txt with 'Hello Sparrow' inside""#.into(),
            expected_output: "file created".into(),
        },
        Lesson {
            id: "swarm".into(),
            title: "The Swarm".into(),
            description: "See Planner → Coder → Verifier work together.".into(),
            command: r#"swarm "Add a comment to hello.txt explaining what it contains""#.into(),
            expected_output: "Planner → Coder → Verifier".into(),
        },
        Lesson {
            id: "autonomy".into(),
            title: "Autonomy Dial".into(),
            description: "Understand how Sparrow keeps you safe with graduated trust.".into(),
            command: r#"run "Read hello.txt" --autonomy supervised"#.into(),
            expected_output: "autonomy gate".into(),
        },
        Lesson {
            id: "skills".into(),
            title: "Self-Improving Skills".into(),
            description: "Sparrow learns from experience and gets better over time.".into(),
            command: "skills list".into(),
            expected_output: "skill library".into(),
        },
        Lesson {
            id: "gateway".into(),
            title: "Everywhere You Are".into(),
            description: "Continue your session from Telegram, Discord, or Slack.".into(),
            command: "gateway status".into(),
            expected_output: "messaging surfaces".into(),
        },
    ]
}

impl Onboarding {
    /// Run interactive tutorial
    pub fn run_interactive(&self) -> anyhow::Result<()> {
        use std::io::{self, Write, BufRead};

        println!("═══ SPARROW LEARN ═══");
        println!("5 interactive lessons to master Sparrow.\n");

        for (i, lesson) in self.lessons.iter().enumerate() {
            println!("── Lesson {}/{} : {} ──", i + 1, self.lessons.len(), lesson.title);
            println!("{}", lesson.description);
            println!("\n  sparrow {}", lesson.command);
            println!("\nPress Enter to continue...");
            io::stdin().lock().lines().next();
            println!();
        }

        println!("═══ All lessons complete! ═══");
        println!("Try: sparrow run 'your own task'");
        println!("Help: sparrow --help\n");
        Ok(())
    }

    /// Detect user mode from config or interactive choice
    pub fn detect_mode(config: &Config) -> UserMode {
        if config.defaults.autonomy == crate::event::AutonomyLevel::Supervised {
            UserMode::Beginner
        } else {
            UserMode::Expert
        }
    }

    /// Generate friendly error messages
    pub fn friendly_error(error_type: &str, context: &str) -> String {
        match error_type {
            "no_provider" => format!(
                "No API key configured.\n→ Run: sparrow auth add <provider>\n→ Or: sparrow setup\n→ Or set env: {}_API_KEY",
                context.to_uppercase()
            ),
            "no_model" => format!(
                "Model '{}' not available.\n→ List models: sparrow model --list\n→ Add one: edit ~/.config/sparrow/config.toml",
                context
            ),
            "budget" => format!(
                "Budget limit reached (${}).\n→ Increase: sparrow run --budget <amount>\n→ Or edit: config.toml [budget]",
                context
            ),
            "sandbox" => format!(
                "Operation blocked by sandbox: {}\n→ Check: sparrow doctor\n→ Change: config.toml [defaults] sandbox",
                context
            ),
            "permission" => format!(
                "Permission denied: {}\n→ In Supervised mode, mutating actions require approval.\n→ Use: --autonomy trusted\n→ Or: sparrow config --edit",
                context
            ),
            _ => format!(
                "Error: {}\n→ Run diagnostics: sparrow doctor\n→ Get help: sparrow --help\n→ Docs: https://github.com/ucav/Sparrow",
                context
            ),
        }
    }

    /// Examples gallery
    pub fn examples_gallery() -> Vec<(&'static str, &'static str)> {
        vec![
            ("run \"fix the failing auth test\"", "Fix a bug"),
            ("run \"add a /health endpoint to the API\"", "Add a feature"),
            ("run \"explain this codebase\" --local", "Analyze offline"),
            ("swarm \"refactor the rate limiter\"", "Swarm review"),
            ("schedule \"run tests\" --cron \"0 */6 * * *\"", "Schedule checks"),
            ("skills list", "See learned skills"),
            ("checkpoint list", "View safety net"),
            ("replay <id>", "Replay any run"),
        ]
    }
}
