// Verifies skills can be removed by name (cleans junk auto-learned skills that
// `prune` — which only touches low-score auto-generated ones — cannot reach).

use sparrow::capabilities::{FsSkillLibrary, Skill, SkillLibrary};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("sparrow-{name}-{id}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mk_skill(name: &str) -> Skill {
    Skill {
        name: name.into(),
        description: "junk".into(),
        trigger: vec!["x".into()],
        body: "body".into(),
        source_file: String::new(),
        usage_count: 0,
        created_at: "2026-01-01".into(),
        score: 0.5,
        auto_generated: false, // user-kind — prune would NOT remove it
        references: Vec::new(),
        templates: Vec::new(),
        scripts: Vec::new(),
        assets: Vec::new(),
    }
}

#[test]
fn remove_deletes_user_skill_that_prune_cannot() {
    let dir = temp_dir("skill-rm");
    let lib = FsSkillLibrary::new(dir.clone());
    lib.add(mk_skill("JunkFromChat")).unwrap();
    assert_eq!(lib.all().len(), 1);

    // prune leaves it (it's a user skill, score >= min)
    let pruned = lib.prune(0.2).unwrap();
    assert_eq!(pruned, 0);
    assert_eq!(lib.all().len(), 1);

    // remove deletes it
    assert!(lib.remove("JunkFromChat").unwrap());
    assert_eq!(lib.all().len(), 0);

    // removing a non-existent skill returns false
    assert!(!lib.remove("Nope").unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}
