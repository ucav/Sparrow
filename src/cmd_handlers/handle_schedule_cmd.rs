// src/cmd_handlers/handle_schedule_cmd.rs
use super::prelude::*;
use super::prelude::*;
pub async fn handle_schedule(
    task: &str,
    cron: &str,
    autonomy: Option<String>,
    _report: &[String],
    scheduler: &Arc<MemoryScheduler>,
) -> anyhow::Result<()> {
    let level = match autonomy.as_deref() {
        Some("autonomous") => sparrow::event::AutonomyLevel::Autonomous,
        Some("trusted") => sparrow::event::AutonomyLevel::Trusted,
        _ => sparrow::event::AutonomyLevel::Supervised,
    };

    // Try natural-language → cron translation before creating the job
    let resolved_cron =
        sparrow::runtime::scheduler::parse_nl_cron(cron).unwrap_or_else(|| cron.to_string());

    if resolved_cron != cron {
        println!("Parsed schedule: \"{}\" → {}", cron, resolved_cron);
    }

    // Validate the cron expression before storing
    {
        use cron::Schedule;
        use std::str::FromStr;
        if Schedule::from_str(&resolved_cron).is_err() {
            anyhow::bail!(
                "Invalid cron expression: '{}'. Use cron syntax (e.g. '0 2 * * *') \
                 or natural language (e.g. 'every day at 2am').",
                resolved_cron
            );
        }
    }

    let mut job = Job::new(task.to_string(), resolved_cron.clone());
    job.autonomy = level.clone();
    job.next_run = job.next_schedule().map(|dt| dt.to_rfc3339());

    let id = scheduler.schedule(job)?;
    let jobs = scheduler.list();

    println!("Job scheduled: {}", id);
    println!("Task    : {}", task);
    println!("Cron    : {}", resolved_cron);
    println!("Autonomy: {:?}", level);

    if let Some(j) = jobs.iter().find(|j| j.id == id) {
        if let Some(next) = &j.next_run {
            println!("Next run: {}", next);
        }
    }

    println!("\nAll scheduled jobs ({}):", jobs.len());
    for j in &jobs {
        let next = j.next_run.as_deref().unwrap_or("pending");
        let status = if j.enabled { "active" } else { "paused" };
        println!("  {} {} | {} | {}", status, j.id, j.cron, next);
    }

    Ok(())
}
