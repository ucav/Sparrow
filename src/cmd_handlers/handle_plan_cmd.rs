// src/cmd_handlers/handle_plan_cmd.rs
pub fn handle_plan(
    task: &str,
    config: &sparrow::config::Config,
    skills: Arc<dyn SkillLibrary>,
    json: bool,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;
    let commands =
        sparrow::commands::all_commands(&project_root, &config.config_dir, Some(skills.as_ref()));
    let plan = sparrow::plan::build_read_only_plan(task, &commands);
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("{}", plan.render_markdown());
    }
    Ok(())
}
