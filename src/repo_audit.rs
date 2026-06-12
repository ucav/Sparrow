use anyhow::Context;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct RepoAudit {
    pub root: String,
    pub files_total: usize,
    pub rust_files: usize,
    pub entrypoints: Vec<String>,
    pub production_stubs: Vec<Finding>,
    pub todo_comments: Vec<Finding>,
    pub suspicious_unlinked_rust_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub text: String,
}

pub fn run_repo_audit(root: &Path) -> anyhow::Result<RepoAudit> {
    let mut files_total = 0usize;
    let mut rust_paths = Vec::new();
    let mut entrypoints = Vec::new();
    let mut production_stubs = Vec::new();
    let mut todo_comments = Vec::new();
    let mut rust_corpus = String::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git"
                    | ".claude"
                    | ".codex"
                    | "target"
                    | "node_modules"
                    | ".next"
                    | "dist"
                    | "build"
            )
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        files_total += 1;
        let path = entry.path();
        let rel = rel_path(root, path);
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        rust_paths.push(path.to_path_buf());
        if rel.ends_with("src/main.rs") || rel.ends_with("src/lib.rs") {
            entrypoints.push(rel.clone());
        }

        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read Rust file {}", path.display()))?;
        rust_corpus.push_str(&text);
        rust_corpus.push('\n');
        let is_test_file = rel.contains("/tests/") || rel.starts_with("tests/");
        for (idx, line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let code_only = strip_string_literals(line);
            if !is_test_file
                && (code_only.contains("todo!()") || code_only.contains("unimplemented!()"))
            {
                production_stubs.push(Finding {
                    file: rel.clone(),
                    line: line_no,
                    text: line.trim().to_string(),
                });
            }
            if line.contains("TODO") || line.contains("FIXME") {
                todo_comments.push(Finding {
                    file: rel.clone(),
                    line: line_no,
                    text: line.trim().to_string(),
                });
            }
        }
    }

    let suspicious_unlinked_rust_files = rust_paths
        .iter()
        .filter_map(|path| {
            let rel = rel_path(root, path);
            let stem = path.file_stem()?.to_string_lossy();
            if matches!(stem.as_ref(), "main" | "lib" | "mod") {
                return None;
            }
            let mod_decl = format!("mod {stem}");
            let namespaced = format!("{stem}::");
            if rust_corpus.contains(&mod_decl) || rust_corpus.contains(&namespaced) {
                None
            } else {
                Some(rel)
            }
        })
        .collect();

    Ok(RepoAudit {
        root: root.display().to_string(),
        files_total,
        rust_files: rust_paths.len(),
        entrypoints,
        production_stubs,
        todo_comments,
        suspicious_unlinked_rust_files,
    })
}

pub fn write_audit_markdown(root: &Path, audit: &RepoAudit) -> anyhow::Result<PathBuf> {
    let dir = root.join("artifacts");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("audit-{stamp}.md"));
    std::fs::write(&path, audit.to_markdown())?;
    Ok(path)
}

impl RepoAudit {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Sparrow Repo Audit\n\n");
        out.push_str(&format!("Root: `{}`\n\n", self.root));
        out.push_str("## Summary\n\n");
        out.push_str(&format!("- Files scanned: {}\n", self.files_total));
        out.push_str(&format!("- Rust files: {}\n", self.rust_files));
        out.push_str(&format!(
            "- Production stubs: {}\n",
            self.production_stubs.len()
        ));
        out.push_str(&format!(
            "- TODO/FIXME comments: {}\n",
            self.todo_comments.len()
        ));
        out.push_str(&format!(
            "- Suspicious unlinked Rust files: {}\n\n",
            self.suspicious_unlinked_rust_files.len()
        ));
        write_list(&mut out, "Entrypoints", &self.entrypoints);
        write_findings(&mut out, "Production Stubs", &self.production_stubs);
        write_findings(&mut out, "TODO/FIXME", &self.todo_comments);
        write_list(
            &mut out,
            "Suspicious Unlinked Rust Files",
            &self.suspicious_unlinked_rust_files,
        );
        out
    }
}

fn write_findings(out: &mut String, title: &str, findings: &[Finding]) {
    out.push_str(&format!("## {title}\n\n"));
    if findings.is_empty() {
        out.push_str("None found.\n\n");
        return;
    }
    for finding in findings {
        out.push_str(&format!(
            "- `{}`:{} — `{}`\n",
            finding.file, finding.line, finding.text
        ));
    }
    out.push('\n');
}

fn write_list(out: &mut String, title: &str, items: &[String]) {
    out.push_str(&format!("## {title}\n\n"));
    if items.is_empty() {
        out.push_str("None found.\n\n");
        return;
    }
    for item in items {
        out.push_str(&format!("- `{item}`\n"));
    }
    out.push('\n');
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn strip_string_literals(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars = line.chars();
    let mut in_string = false;
    let mut escaped = false;
    for ch in chars {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(' ');
        } else if ch == '"' {
            in_string = true;
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_audit_finds_stubs_and_entrypoints() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("main.rs"),
            "mod used;\nfn main(){ used::go(); }\n// TODO: wire cli\n",
        )
        .unwrap();
        std::fs::write(src.join("used.rs"), "pub fn go() {}\n").unwrap();
        std::fs::write(src.join("lonely.rs"), "pub fn nope(){ todo!() }\n").unwrap();

        let audit = run_repo_audit(temp.path()).unwrap();
        assert_eq!(audit.entrypoints, vec!["src/main.rs"]);
        assert_eq!(audit.production_stubs.len(), 1);
        assert_eq!(audit.todo_comments.len(), 1);
        assert!(
            audit
                .suspicious_unlinked_rust_files
                .contains(&"src/lonely.rs".to_string())
        );
        assert!(audit.to_markdown().contains("Production Stubs"));
    }

    #[test]
    fn repo_audit_ignores_stub_words_inside_strings() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("main.rs"), "fn main(){ println!(\"todo!()\"); }\n").unwrap();
        let audit = run_repo_audit(temp.path()).unwrap();
        assert!(audit.production_stubs.is_empty());
    }
}
