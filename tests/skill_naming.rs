use sparrow::capabilities::{Curator, FsSkillLibrary, Skill, SkillLibrary};

#[test]
fn specific_task_never_generates_skill() {
    let result = Curator::propose_skill("analyse le repo github ucav/Sparrow", "done");
    assert!(
        result.is_none(),
        "specific task should never generate a skill"
    );
}

#[test]
fn short_task_never_generates_skill() {
    let result = Curator::propose_skill("fix bug", "fixed");
    assert!(result.is_none());
}

#[test]
fn pattern_task_generates_correct_name() {
    let result = Curator::propose_skill(
        "write unit tests for the authentication module",
        "added 12 tests, all passing",
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "write-and-fix-tests");
}

#[test]
fn duplicate_skill_not_proposed() {
    let root = std::env::temp_dir().join(format!(
        "sparrow-skill-naming-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let library = FsSkillLibrary::new(root.clone());
    library
        .add(Skill {
            name: "write-and-fix-tests".into(),
            description: "existing".into(),
            trigger: vec!["test".into()],
            body: "existing body".into(),
            source_file: String::new(),
            usage_count: 0,
            created_at: "2026-05-31".into(),
            score: 0.8,
            auto_generated: false,
        })
        .unwrap();

    let result = Curator::propose_skill_if_missing(
        "write unit tests for the authentication module",
        "added 12 tests, all passing",
        &library,
    );
    assert!(result.is_none());

    let _ = std::fs::remove_dir_all(root);
}
