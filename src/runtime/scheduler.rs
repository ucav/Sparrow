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

    pub fn start_cron_loop(
        self: &Arc<Self>,
        engine: Arc<Engine>,
        recorder: Arc<FsRecorder>,
    ) -> tokio::task::JoinHandle<()> {
        let scheduler = self.clone();
        tokio::spawn(async move {
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
        })
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
