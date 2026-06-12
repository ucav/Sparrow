use anyhow::Context;
use std::path::{Path, PathBuf};

pub fn prepare_release_docs(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let launch_dir = root.join("docs").join("launch");
    std::fs::create_dir_all(&launch_dir)?;
    let ring_release = launch_dir.join("ring-release.md");
    let migration = root.join("docs").join("migration-v0.9.2.md");
    std::fs::create_dir_all(migration.parent().unwrap_or(root))?;

    let perf = read_optional(root.join("artifacts").join("perf-report.md"));
    let changelog = read_optional(root.join("CHANGELOG.md"));
    std::fs::write(&ring_release, ring_release_markdown(&perf, &changelog))
        .with_context(|| format!("failed to write {}", ring_release.display()))?;
    std::fs::write(&migration, migration_markdown())
        .with_context(|| format!("failed to write {}", migration.display()))?;

    Ok(vec![ring_release, migration])
}

pub fn planned_release_doc_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("docs").join("launch").join("ring-release.md"),
        root.join("docs").join("migration-v0.9.2.md"),
    ]
}

fn ring_release_markdown(perf: &str, changelog: &str) -> String {
    format!(
        "# Sparrow v0.9.2 \"The Ring\"\n\n\
         ## Demo 30s\n\n\
         1. Run `sparrow audit` to map the repo and catch stubs.\n\
         2. Run `sparrow test` to detect and execute the project runner.\n\
         3. Open `sparrow console --fast` for a lean cockpit startup.\n\n\
         ## Measured Before/After\n\n\
         The release notes use only measured values from `artifacts/perf-report.md`.\n\n\
         ```text\n{}\n```\n\n\
         ## What Changed\n\n\
         ```text\n{}\n```\n\n\
         ## Install\n\n\
         ```powershell\ncargo install sparrow-cli\n```\n",
        excerpt(perf, 5000),
        excerpt(changelog, 4000)
    )
}

fn migration_markdown() -> &'static str {
    "# Migration Notes: Sparrow v0.9.2\n\n\
     No breaking config migration is expected for v0.9.2.\n\n\
     - Existing `config.toml` files continue to load.\n\
     - Existing transcripts remain additive-serde compatible.\n\
     - `sparrow console` keeps normal browser auto-open behavior.\n\
     - `sparrow console --fast` is new and intentionally does not auto-open a browser.\n"
}

fn read_optional(path: PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| "(not available yet)".into())
}

fn excerpt(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_prep_writes_expected_docs() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("artifacts")).unwrap();
        std::fs::write(temp.path().join("artifacts/perf-report.md"), "perf values").unwrap();
        std::fs::write(temp.path().join("CHANGELOG.md"), "change values").unwrap();
        let files = prepare_release_docs(temp.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("ring-release.md"));
        assert!(
            std::fs::read_to_string(&files[0])
                .unwrap()
                .contains("perf values")
        );
        assert!(
            std::fs::read_to_string(&files[1])
                .unwrap()
                .contains("No breaking config migration")
        );
    }
}
