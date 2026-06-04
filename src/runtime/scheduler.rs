use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::engine::{Engine, Task};
use crate::event::AutonomyLevel;
use crate::memory::Memory;
use crate::runtime::recorder::{FsRecorder, Recorder, RunInputs};

// ─── Job ────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub task: String,
    pub cron: String,
    pub autonomy: AutonomyLevel,
    pub sandbox: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
}

impl Job {
    pub fn new(task: String, cron: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task,
            cron,
            autonomy: AutonomyLevel::Supervised,
            sandbox: "local-hardened".into(),
            enabled: true,
            last_run: None,
            next_run: None,
            created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    /// Uses the `cron` crate for full cron expression support.
    pub fn next_schedule(&self) -> Option<DateTime<Utc>> {
        use cron::Schedule;
        use std::str::FromStr;
        Schedule::from_str(&self.cron)
            .ok()
            .and_then(|s| s.upcoming(Utc).next())
    }
}

// ─── Natural-language → cron parser ────────────────────────────────────────────

/// Try to convert a natural-language schedule expression into a cron string.
/// Examples:
///   "every minute"            → "* * * * *"
///   "every hour" / "hourly"   → "0 * * * *"
///   "every day at 2am"        → "0 2 * * *"
///   "daily at 9:30"           → "30 9 * * *"
///   "every monday at 8"       → "0 8 * * 1"
///   "every 15 minutes"        → "*/15 * * * *"
///   "midnight"                → "0 0 * * *"
///   "noon"                    → "0 12 * * *"
///   "weekly"                  → "0 9 * * 1"
///   "monthly"                 → "0 9 1 * *"
///
/// Returns `None` if the input already looks like a cron expression or cannot be parsed.
pub fn parse_nl_cron(input: &str) -> Option<String> {
    // Already a cron expression (5 whitespace-separated tokens, first can be */N or digits)
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() == 5 {
        return None; // looks like raw cron, leave as-is
    }

    let lower = input.to_lowercase();
    let lower = lower.trim();

    // Named shortcuts
    if lower == "hourly" || lower == "every hour" {
        return Some("0 * * * *".into());
    }
    if lower == "daily" || lower == "every day" {
        return Some("0 9 * * *".into());
    }
    if lower == "weekly" || lower == "every week" {
        return Some("0 9 * * 1".into());
    }
    if lower == "monthly" || lower == "every month" {
        return Some("0 9 1 * *".into());
    }
    if lower.contains("midnight") {
        return Some("0 0 * * *".into());
    }
    if lower.contains("noon") {
        return Some("0 12 * * *".into());
    }
    if lower == "every minute" {
        return Some("* * * * *".into());
    }

    // "every N minutes"
    if let Some(n_str) = lower
        .strip_prefix("every ")
        .and_then(|s| s.strip_suffix(" minutes"))
    {
        if let Ok(n) = n_str.trim().parse::<u32>() {
            if n > 0 && n < 60 {
                return Some(format!("*/{} * * * *", n));
            }
        }
    }
    if let Some(n_str) = lower
        .strip_prefix("every ")
        .and_then(|s| s.strip_suffix(" hours"))
    {
        if let Ok(n) = n_str.trim().parse::<u32>() {
            if n > 0 && n <= 24 {
                return Some(format!("0 */{} * * *", n));
            }
        }
    }

    // Day-of-week mapping
    let dow_map = [
        ("sunday", 0),
        ("monday", 1),
        ("tuesday", 2),
        ("wednesday", 3),
        ("thursday", 4),
        ("friday", 5),
        ("saturday", 6),
        ("sun", 0),
        ("mon", 1),
        ("tue", 2),
        ("wed", 3),
        ("thu", 4),
        ("fri", 5),
        ("sat", 6),
    ];

    // Parse optional hour from "at H", "at H:MM", "at Ham/pm"
    fn parse_hour_minute(at_str: &str) -> Option<(u32, u32)> {
        let s = at_str
            .trim()
            .trim_start_matches("at ")
            .replace("am", "")
            .replace("pm", "");
        let is_pm = at_str.contains("pm");
        if let Some((h, m)) = s.split_once(':') {
            let h: u32 = h.trim().parse().ok()?;
            let m: u32 = m.trim().parse().ok()?;
            let h = if is_pm && h < 12 {
                h + 12
            } else if !is_pm && h == 12 {
                0
            } else {
                h
            };
            Some((h.min(23), m.min(59)))
        } else {
            let h: u32 = s.trim().parse().ok()?;
            let h = if is_pm && h < 12 {
                h + 12
            } else if !is_pm && h == 12 {
                0
            } else {
                h
            };
            Some((h.min(23), 0))
        }
    }

    // "every <day> at <time>"
    for (day_name, dow) in &dow_map {
        if lower.contains(day_name) {
            let at_idx = lower.find(" at ");
            let (h, m) = if let Some(idx) = at_idx {
                parse_hour_minute(&lower[idx + 4..]).unwrap_or((9, 0))
            } else {
                (9, 0)
            };
            return Some(format!("{} {} * * {}", m, h, dow));
        }
    }

    // "every day at <time>" / "daily at <time>"
    if lower.contains("every day") || lower.contains("daily") {
        if let Some(idx) = lower.find(" at ") {
            let (h, m) = parse_hour_minute(&lower[idx + 4..]).unwrap_or((9, 0));
            return Some(format!("{} {} * * *", m, h));
        }
        return Some("0 9 * * *".into());
    }

    // "every hour at :MM" / "every hour at MM"
    if lower.contains("every hour") {
        if let Some(idx) = lower.find(" at ") {
            let time_part = lower[idx + 4..].trim();
            let m: u32 = time_part.trim_start_matches(':').parse().unwrap_or(0);
            return Some(format!("{} * * * *", m.min(59)));
        }
        return Some("0 * * * *".into());
    }

    None
}

// ─── THE SCHEDULER TRAIT ────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Scheduler: Send + Sync {
    fn schedule(&self, job: Job) -> anyhow::Result<String>;
    fn list(&self) -> Vec<Job>;
    fn cancel(&self, id: &str) -> anyhow::Result<()>;
    fn get(&self, id: &str) -> Option<Job>;
    /// Run all due jobs
    async fn tick(&self) -> Vec<Job>;
}

// ─── In-memory scheduler (M4) — persists to memory via WorkingDocs ──────────────

pub struct MemoryScheduler {
    jobs: std::sync::Mutex<Vec<Job>>,
    memory: Option<Arc<dyn Memory>>,
    /// Abort handle for the running cron loop, if any. Repeated
    /// `start_cron_loop()` calls cancel the previous loop instead of leaking
    /// parallel copies (each loop would otherwise re-fire every due job).
    cron_abort: std::sync::Mutex<Option<tokio::task::AbortHandle>>,
}

impl MemoryScheduler {
    pub fn new() -> Self {
        Self {
            jobs: std::sync::Mutex::new(Vec::new()),
            memory: None,
            cron_abort: std::sync::Mutex::new(None),
        }
    }

    pub fn with_memory(mut self, memory: Arc<dyn Memory>) -> Self {
        self.memory = Some(memory);
        self
    }

    fn persist_jobs_sync(&self) {
        if let Some(mem) = &self.memory {
            let jobs = self.jobs.lock().unwrap();
            if let Ok(json) = serde_json::to_string_pretty(&*jobs) {
                let _ = mem.upsert_doc(crate::memory::WorkingDoc {
                    id: "scheduler-jobs".into(),
                    title: "Scheduled Jobs".into(),
                    content: json,
                    updated_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                });
            }
        }
    }

    pub fn restore_sync(&self) {
        if let Some(mem) = &self.memory {
            let docs = mem.shared_docs();
            if let Some(doc) = docs.iter().find(|d| d.id == "scheduler-jobs") {
                if let Ok(jobs) = serde_json::from_str::<Vec<Job>>(&doc.content) {
                    let mut guard = self.jobs.lock().unwrap();
                    *guard = jobs;
                }
            }
        }
    }

    /// Stop the running cron loop if any. Idempotent.
    pub fn stop_cron_loop(&self) {
        if let Some(abort) = self.cron_abort.lock().unwrap().take() {
            abort.abort();
        }
    }

    pub fn start_cron_loop(
        self: &Arc<Self>,
        engine: Arc<Engine>,
        recorder: Arc<FsRecorder>,
    ) -> tokio::task::JoinHandle<()> {
        // Cancel any previously-running loop so repeated start() calls don't
        // leak parallel ticking loops (each tick spawns engine work — multiple
        // loops would fire the same job multiple times).
        if let Some(prev) = self.cron_abort.lock().unwrap().take() {
            prev.abort();
        }
        let scheduler = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                let due_jobs = scheduler.tick().await;
                for job in due_jobs {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    let task = Task {
                        description: job.task.clone(),
                        context: vec![],
                    };
                    let run_id = uuid::Uuid::new_v4().to_string();
                    recorder.start_run(
                        run_id.clone(),
                        RunInputs {
                            task: job.task.clone(),
                            config_snapshot: serde_json::json!({}),
                            model_id: "scheduled".into(),
                            repo_head: None,
                            timestamp: Utc::now().to_rfc3339(),
                            agent: "scheduler".into(),
                        },
                    );

                    let engine_clone = engine.clone();
                    let recorder_clone = recorder.clone();
                    tokio::spawn(async move {
                        let engine_run_id = run_id.clone();
                        let engine_handle = tokio::spawn(async move {
                            engine_clone
                                .drive_with_run_id(task, tx, crate::event::RunId(engine_run_id))
                                .await
                        });
                        while let Some(event) = rx.recv().await {
                            recorder_clone.record(&event);
                        }
                        if let Err(err) = engine_handle.await {
                            tracing::error!("scheduled engine task failed: {}", err);
                        }
                        let _ = recorder_clone.finalize(&run_id);
                    });
                }
            }
        });
        // Store an AbortHandle so the next start() or an explicit stop() can
        // cancel this loop. The original JoinHandle is still returned to the
        // caller for backward compatibility.
        *self.cron_abort.lock().unwrap() = Some(handle.abort_handle());
        handle
    }
}

#[async_trait::async_trait]
impl Scheduler for MemoryScheduler {
    fn schedule(&self, job: Job) -> anyhow::Result<String> {
        let id = job.id.clone();
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push(job);
        drop(jobs);
        self.persist_jobs_sync();
        Ok(id)
    }

    fn list(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().clone()
    }

    fn cancel(&self, id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| j.id != id);
        drop(jobs);
        self.persist_jobs_sync();
        Ok(())
    }

    fn get(&self, id: &str) -> Option<Job> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == id)
            .cloned()
    }

    async fn tick(&self) -> Vec<Job> {
        let now = Utc::now();
        let mut due = Vec::new();
        let mut jobs = self.jobs.lock().unwrap();

        for job in jobs.iter_mut() {
            if !job.enabled {
                continue;
            }
            if let Some(next) = &job.next_run {
                if let Ok(next_dt) = DateTime::parse_from_rfc3339(next) {
                    if next_dt <= now {
                        due.push(job.clone());
                        job.last_run = Some(now.to_rfc3339());
                        job.next_run = job.next_schedule().map(|dt| dt.to_rfc3339());
                    }
                }
            } else {
                job.next_run = job.next_schedule().map(|dt| dt.to_rfc3339());
            }
        }

        drop(jobs);
        self.persist_jobs_sync();
        due
    }
}

impl Default for MemoryScheduler {
    fn default() -> Self {
        Self::new()
    }
}
