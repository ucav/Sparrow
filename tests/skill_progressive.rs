use sparrow::capabilities::{FsSkillLibrary, SkillLibrary};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sparrow-{name}-{id}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn references_load_only_when_skill_is_invoked() {
    let root = temp_dir("skill-progressive");
    let skill_dir = root.join("deep-review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Skill: deep-review\n\n**Trigger:** review\n\n**Description:** Review code.\n\n**References:** references/checklist.md\n\n## Body\nShort operating instructions.",
    )
    .unwrap();
    std::fs::create_dir_all(skill_dir.join("references")).unwrap();
    std::fs::write(
        skill_dir.join("references/checklist.md"),
        "Detailed checklist loaded on demand.",
    )
    .unwrap();

    let lib = FsSkillLibrary::new(root.clone());
    let relevant = lib.relevant("please review this module", 1);
    assert_eq!(relevant.len(), 1);
    assert!(!relevant[0].body.contains("Detailed checklist"));

    let invocation = lib.invoke("deep-review").unwrap().unwrap();
    assert_eq!(invocation.loaded_references.len(), 1);
    assert!(
        invocation.loaded_references[0]
            .1
            .contains("Detailed checklist")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invoke_loads_templates_scripts_and_assets() {
    let root = temp_dir("skill-full-progressive");
    let skill_dir = root.join("full-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Skill: full-skill\n\n**Trigger:** test\n\n**Description:** Full progressive skill.\n\n**References:** refs/guide.md\n\n**Templates:** tmpl/scaffold.rs\n\n**Scripts:** scripts/run.sh\n\n**Assets:** assets/config.toml\n\n## Body\nShort instructions.",
    )
    .unwrap();
    std::fs::create_dir_all(skill_dir.join("refs")).unwrap();
    std::fs::create_dir_all(skill_dir.join("tmpl")).unwrap();
    std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
    std::fs::create_dir_all(skill_dir.join("assets")).unwrap();
    std::fs::write(skill_dir.join("refs/guide.md"), "Reference guide content.").unwrap();
    std::fs::write(skill_dir.join("tmpl/scaffold.rs"), "fn main() {}").unwrap();
    std::fs::write(skill_dir.join("scripts/run.sh"), "#!/bin/bash\necho hello").unwrap();
    std::fs::write(
        skill_dir.join("assets/config.toml"),
        "[settings]\nkey = \"value\"",
    )
    .unwrap();

    let lib = FsSkillLibrary::new(root.clone());
    let invocation = lib.invoke("full-skill").unwrap().unwrap();

    assert_eq!(invocation.loaded_references.len(), 1);
    assert!(
        invocation.loaded_references[0]
            .1
            .contains("Reference guide")
    );

    assert_eq!(invocation.loaded_templates.len(), 1);
    assert!(invocation.loaded_templates[0].1.contains("fn main()"));

    assert_eq!(invocation.loaded_scripts.len(), 1);
    assert!(invocation.loaded_scripts[0].1.contains("echo hello"));

    assert_eq!(invocation.loaded_assets.len(), 1);
    assert!(invocation.loaded_assets[0].1.contains("key = \"value\""));

    let _ = std::fs::remove_dir_all(root);
}
