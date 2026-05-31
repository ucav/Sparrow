use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::event::AutonomyLevel;
use crate::memory::Memory;

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

    /// Simple cron parsing: minute hour day-of-month month day-of-week
    /// Returns next scheduled DateTime.
    pub fn next_schedule(&self) -> Option<DateTime<Utc>> {
        parse_cron(&self.cron)
    }
}

/// Very basic cron parser: "minute hour day month weekday"
/// Supports `*` and `*/N` for minute and hour fields.
fn parse_cron(expr: &str) -> Option<DateTime<Utc>> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let now = Utc::now();
    let minute = parse_field(parts[0], now.minute());
    let hour = parse_field(parts[1], now.hour());

    // Build next occurrence: if already past today, schedule for next hour/day
    let mut next = now
        .date_naive()
        .and_hms_opt(hour as u32, minute as u32, 0)?
        .and_local_timezone(Utc)
        .single()?;

    if next <= now {
        // Schedule for next hour
        next = now
            .date_naive()
            .and_hms_opt(now.hour() + 1, minute as u32, 0)
            .or_else(|| {
                now.date_naive()
                    .succ_opt()?
                    .and_hms_opt(0, minute as u32, 0)
            })?
            .and_local_timezone(Utc)
            .single()?;
    }

    Some(next)
}

fn parse_field(field: &str, current: u32) -> u32 {
    if field == "*" {
        return current;
    }
    if let Some(step) = field.strip_prefix("*/") {
        let step: u32 = step.parse().unwrap_or(1);
        return ((current / step) + 1) * step;
    }
    field.parse().unwrap_or(current)
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
}

impl MemoryScheduler {
    pub fn new() -> Self {
        Self {
            jobs: std::sync::Mutex::new(Vec::new()),
            memory: None,
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
        self.jobs.lock().unwrap().iter().find(|j| j.id == id).cloned()
    }

    async fn tick(&self) -> Vec<Job> {
        let now = Utc::now();
        let mut due = Vec::new();
        let mut jobs = self.jobs.lock().unwrap();

        for job in jobs.iter_mut() {
            if !job.enabled { continue; }
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
