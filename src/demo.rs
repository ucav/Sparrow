//! `sparrow demo` — A self-contained demo that codes a snake game.
//!
//! Shows live progress with module labels ("[Planner] Analyzing...",
//! "[Coder] Writing game.rs...", "[Verifier] Checking...") using colored
//! terminal output. Takes ~30 seconds and is designed to be shared on
//! social media to showcase Sparrow's swarm pipeline.

use std::io::{self, Write};
use std::time::Duration;

/// Run the self-contained demo.
///
/// Simulates Sparrow's Planner → Coder → Verifier pipeline by generating
/// a simple snake game in Rust, compiling it, and optionally running it.
pub async fn run_demo(skills: Option<&dyn crate::capabilities::SkillLibrary>) -> anyhow::Result<()> {
    use crossterm::style::{Color, Print, SetForegroundColor, ResetColor};
    use crossterm::ExecutableCommand;

    let mut stdout = io::stdout();

    // ── Header ────────────────────────────────────────────────────────
    println!();
    stdout.execute(SetForegroundColor(Color::Cyan))?;
    println!("══════════════════════════════════════════════════");
    println!("  🐦 SPARROW DEMO — Let's code a Snake game !");
    println!("══════════════════════════════════════════════════");
    stdout.execute(ResetColor)?;
    println!();

    // ── Phase 1: Planner ──────────────────────────────────────────────
    phase_header(&mut stdout, "Planner", "Analyse de la demande...", Color::Yellow)?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let plan = vec![
        "1. Créer un fichier game.rs avec la boucle de jeu",
        "2. Implémenter le serpent (position, direction, croissance)",
        "3. Génération de la pomme aléatoire",
        "4. Gestion des entrées clavier (flèches directionnelles)",
        "5. Détection de collision (murs + auto-collision)",
        "6. Affichage terminal avec crossterm",
    ];

    for step in &plan {
        print!("  ");
        stdout.execute(SetForegroundColor(Color::DarkYellow))?;
        print!("→ ");
        stdout.execute(ResetColor)?;
        println!("{step}");
        io::stdout().flush().ok();
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    println!();

    // ── Phase 2: Coder ────────────────────────────────────────────────
    phase_header(&mut stdout, "Coder", "Écriture de game.rs...", Color::Green)?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let game_code = generate_snake_game_code();

    // Show code being "written" with typing effect (truncated for speed)
    let preview_lines: Vec<&str> = game_code.lines().take(8).collect();
    for line in &preview_lines {
        stdout.execute(SetForegroundColor(Color::DarkGreen))?;
        println!("  │ {line}");
        io::stdout().flush().ok();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    stdout.execute(SetForegroundColor(Color::DarkGreen))?;
    println!("  │ ... ({} lignes au total)", game_code.lines().count());
    stdout.execute(ResetColor)?;

    // Write the actual file
    let demo_dir = std::env::temp_dir().join("sparrow_demo");
    std::fs::create_dir_all(&demo_dir)?;
    let game_path = demo_dir.join("game.rs");
    std::fs::write(&game_path, &game_code)?;

    println!();
    stdout.execute(SetForegroundColor(Color::Green))?;
    println!("  ✓ Fichier écrit → {}", game_path.display());
    stdout.execute(ResetColor)?;
    println!();

    // ── Phase 3: Verifier ─────────────────────────────────────────────
    phase_header(&mut stdout, "Verifier", "Vérification du code...", Color::Magenta)?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Try to compile the game
    let compile_result = compile_snake_game(&demo_dir, &game_path);
    match &compile_result {
        Ok(()) => {
            stdout.execute(SetForegroundColor(Color::Green))?;
            println!("  ✓ Compilation réussie !");
            stdout.execute(ResetColor)?;
        }
        Err(err) => {
            stdout.execute(SetForegroundColor(Color::Red))?;
            println!("  ✗ Compilation échouée : {err}");
            stdout.execute(ResetColor)?;
            println!("  → Le code source reste disponible dans {}", demo_dir.display());
        }
    }

    println!();

    // ── Summary ───────────────────────────────────────────────────────
    stdout.execute(SetForegroundColor(Color::Cyan))?;
    println!("══════════════════════════════════════════════════");
    println!("  🎉 Démo terminée !");
    println!("══════════════════════════════════════════════════");
    stdout.execute(ResetColor)?;
    println!();
    println!("  Planner  : {} étapes planifiées", plan.len());
    println!("  Coder    : {} lignes de code générées", game_code.lines().count());
    println!(
        "  Verifier : {}",
        if compile_result.is_ok() {
            "compilation OK ✓"
        } else {
            "compilation échouée ✗"
        }
    );
    println!();
    println!("  Code source : {}", game_path.display());
    println!();

    // ── Offer to run ──────────────────────────────────────────────────
    if compile_result.is_ok() {
        println!("🐍 Le jeu est prêt ! Veux-tu y jouer maintenant ?");
        print!("Lancer le snake game ? [O/n] ");
        io::stdout().flush().ok();

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        if !matches!(answer.trim().to_lowercase().as_str(), "n" | "no" | "non") {
            run_compiled_game(&demo_dir)?;
        }
    }

    println!("\n✨ Merci d'avoir testé Sparrow !");
    println!("   → Partage : sparrow share");
    println!("   → Docs    : https://github.com/ucav/Sparrow\n");

    Ok(())
}

// ─── Phase header helper ─────────────────────────────────────────────────────

fn phase_header(
    stdout: &mut io::Stdout,
    label: &str,
    subtitle: &str,
    color: crossterm::style::Color,
) -> io::Result<()> {
    use crossterm::style::{Attribute, Print, SetAttribute, SetForegroundColor, ResetColor};
    use crossterm::ExecutableCommand;

    stdout.execute(SetForegroundColor(color))?;
    stdout.execute(SetAttribute(Attribute::Bold))?;
    print!("[{label}]");
    stdout.execute(ResetColor)?;
    stdout.execute(SetAttribute(Attribute::Reset))?;
    println!(" {subtitle}");
    Ok(())
}

// ─── Snake game code generator ────────────────────────────────────────────────

/// Generate a complete, compilable snake game in Rust using crossterm.
fn generate_snake_game_code() -> String {
    r#"//! Snake Game — Généré par Sparrow Demo
//!
//! Un snake game minimaliste dans le terminal.
//! Contrôles : ← ↑ ↓ → (flèches directionnelles), q pour quitter.

use std::collections::VecDeque;
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode};
use crossterm::style::{Color, Print, SetForegroundColor, ResetColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{ExecutableCommand, QueueableCommand};
use rand::Rng;

const WIDTH: u16 = 40;
const HEIGHT: u16 = 20;
const TICK_MS: u64 = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

struct Game {
    snake: VecDeque<(u16, u16)>,
    direction: Direction,
    food: (u16, u16),
    score: u32,
    game_over: bool,
}

impl Game {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let start_x = WIDTH / 2;
        let start_y = HEIGHT / 2;
        let mut snake = VecDeque::new();
        snake.push_back((start_x, start_y));
        snake.push_back((start_x - 1, start_y));
        snake.push_back((start_x - 2, start_y));

        let mut game = Self {
            snake,
            direction: Direction::Right,
            food: (rng.gen_range(1..WIDTH - 1), rng.gen_range(1..HEIGHT - 1)),
            score: 0,
            game_over: false,
        };
        game.spawn_food();
        game
    }

    fn spawn_food(&mut self) {
        let mut rng = rand::thread_rng();
        loop {
            let candidate = (rng.gen_range(1..WIDTH - 1), rng.gen_range(1..HEIGHT - 1));
            if !self.snake.contains(&candidate) {
                self.food = candidate;
                break;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }

        let head = *self.snake.front().unwrap();
        let new_head = match self.direction {
            Direction::Up => (head.0, head.1.wrapping_sub(1)),
            Direction::Down => (head.0, head.1 + 1),
            Direction::Left => (head.0.wrapping_sub(1), head.1),
            Direction::Right => (head.0 + 1, head.1),
        };

        // Wall collision
        if new_head.0 == 0 || new_head.0 >= WIDTH - 1 || new_head.1 == 0 || new_head.1 >= HEIGHT - 1
        {
            self.game_over = true;
            return;
        }

        // Self collision
        if self.snake.contains(&new_head) {
            self.game_over = true;
            return;
        }

        self.snake.push_front(new_head);

        if new_head == self.food {
            self.score += 1;
            self.spawn_food();
        } else {
            self.snake.pop_back();
        }
    }

    fn set_direction(&mut self, dir: Direction) {
        if dir != self.direction.opposite() {
            self.direction = dir;
        }
    }

    fn draw(&self, stdout: &mut std::io::Stdout) {
        stdout.queue(Clear(ClearType::All)).unwrap();

        // Draw top wall
        stdout.queue(crossterm::cursor::MoveTo(0, 0)).unwrap();
        stdout
            .queue(SetForegroundColor(Color::DarkBlue))
            .unwrap();
        for _ in 0..WIDTH {
            stdout.queue(Print("█")).unwrap();
        }

        // Draw body
        for y in 1..HEIGHT {
            stdout.queue(crossterm::cursor::MoveTo(0, y)).unwrap();
            stdout
                .queue(SetForegroundColor(Color::DarkBlue))
                .unwrap();
            stdout.queue(Print("█")).unwrap(); // left wall

            for x in 1..WIDTH - 1 {
                if self.snake.contains(&(x, y)) {
                    let is_head = self.snake.front() == Some(&(x, y));
                    stdout
                        .queue(SetForegroundColor(if is_head {
                            Color::Green
                        } else {
                            Color::DarkGreen
                        }))
                        .unwrap();
                    stdout.queue(Print(if is_head { "●" } else { "○" })).unwrap();
                } else if (x, y) == self.food {
                    stdout.queue(SetForegroundColor(Color::Red)).unwrap();
                    stdout.queue(Print("🍎")).unwrap();
                } else {
                    stdout.queue(Print(" ")).unwrap();
                }
            }

            stdout
                .queue(SetForegroundColor(Color::DarkBlue))
                .unwrap();
            stdout.queue(Print("█")).unwrap(); // right wall
        }

        // Draw bottom wall
        stdout.queue(crossterm::cursor::MoveTo(0, HEIGHT)).unwrap();
        stdout
            .queue(SetForegroundColor(Color::DarkBlue))
            .unwrap();
        for _ in 0..WIDTH {
            stdout.queue(Print("█")).unwrap();
        }

        // Score
        stdout
            .queue(crossterm::cursor::MoveTo(0, HEIGHT + 1))
            .unwrap();
        stdout.queue(ResetColor).unwrap();
        stdout
            .queue(Print(format!(
                "Score: {}  |  Flèches: diriger  |  q: quitter",
                self.score
            )))
            .unwrap();

        stdout.flush().unwrap();
    }
}

fn main() -> anyhow::Result<()> {
    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    stdout.execute(Hide)?;
    stdout.execute(Clear(ClearType::All))?;

    let mut game = Game::new();
    let mut last_tick = Instant::now();

    loop {
        // Handle input
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = event::read()? {
                match key_event.code {
                    KeyCode::Up => game.set_direction(Direction::Up),
                    KeyCode::Down => game.set_direction(Direction::Down),
                    KeyCode::Left => game.set_direction(Direction::Left),
                    KeyCode::Right => game.set_direction(Direction::Right),
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        terminal::disable_raw_mode()?;
                        stdout.execute(Show)?;
                        println!("\n👋 Score final : {}\n", game.score);
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        // Game tick
        if last_tick.elapsed() >= Duration::from_millis(TICK_MS) {
            game.tick();
            last_tick = Instant::now();
            game.draw(&mut stdout);

            if game.game_over {
                stdout
                    .queue(crossterm::cursor::MoveTo(WIDTH / 2 - 5, HEIGHT / 2))
                    .unwrap();
                stdout.queue(SetForegroundColor(Color::Red)).unwrap();
                stdout.queue(Print("GAME OVER!")).unwrap();
                stdout.queue(ResetColor).unwrap();
                stdout.flush().unwrap();
                std::thread::sleep(Duration::from_secs(2));

                terminal::disable_raw_mode()?;
                stdout.execute(Show)?;
                println!("\n💀 Game Over ! Score final : {}\n", game.score);
                return Ok(());
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
"#
    .to_string()
}

// ─── Compilation ─────────────────────────────────────────────────────────────

/// Try to compile the generated snake game.
fn compile_snake_game(demo_dir: &std::path::Path, game_path: &std::path::Path) -> anyhow::Result<()> {
    // Check if rustc is available
    let rustc_check = std::process::Command::new("rustc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if rustc_check.map_or(true, |s| !s.success()) {
        anyhow::bail!("rustc n'est pas installé. Installe Rust : https://rustup.rs");
    }

    // Create a minimal Cargo.toml for the demo (crossterm + rand deps)
    let cargo_toml = format!(
        r#"[package]
name = "sparrow-snake"
version = "0.1.0"
edition = "2021"

[dependencies]
crossterm = "0.28"
rand = "0.8"
anyhow = "1"
"#
    );
    std::fs::write(demo_dir.join("Cargo.toml"), cargo_toml)?;

    // Rename game.rs to main.rs in a src/ directory
    let src_dir = demo_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::copy(game_path, src_dir.join("main.rs"))?;

    // Run cargo build
    let output = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(demo_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Compilation échouée : {stderr}");
    }

    Ok(())
}

/// Run the compiled snake game.
fn run_compiled_game(demo_dir: &std::path::Path) -> anyhow::Result<()> {
    let binary = demo_dir.join("target/release/sparrow-snake");

    if !binary.exists() {
        anyhow::bail!("Binaire introuvable : {}", binary.display());
    }

    println!("\n🐍 Lancement du jeu... (q pour quitter)\n");

    let status = std::process::Command::new(&binary)
        .current_dir(demo_dir)
        .status()?;

    if !status.success() {
        anyhow::bail!("Le jeu a terminé avec une erreur.");
    }

    Ok(())
}
