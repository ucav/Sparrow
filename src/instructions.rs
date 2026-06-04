use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_INSTRUCTION_FILE_BYTES: usize = 16_000;
const MAX_TOTAL_INSTRUCTION_BYTES: usize = 48_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstructionScope {
    User,
    Workspace,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionDoc {
    pub scope: InstructionScope,
    pub path: PathBuf,
    pub relative_path: String,
    pub depth: usize,
    pub content: String,
}

pub fn discover_workspace_instructions(root: &Path, task: &str) -> Vec<InstructionDoc> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut docs = Vec::new();
    let mut seen = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        for path in instruction_candidates_in_dir(&home, false) {
            push_doc(&root, &path, InstructionScope::User, &mut seen, &mut docs);
        }
    }

    for path in instruction_candidates_in_dir(&root, true) {
        push_doc(
            &root,
            &path,
            InstructionScope::Workspace,
            &mut seen,
            &mut docs,
        );
    }

    scan_instruction_dirs(&root, &root, &mut seen, &mut docs);
    sort_instruction_docs(&root, task, &mut docs);
    enforce_total_limit(docs)
}

fn instruction_candidates_in_dir(dir: &Path, include_sparrow: bool) -> Vec<PathBuf> {
    let mut out = vec![dir.join("AGENTS.md"), dir.join("CLAUDE.md")];
    if dir.file_name().and_then(|name| name.to_str()) == Some(".sparrow") {
        out.push(dir.join("INSTRUCTIONS.md"));
    }
    if include_sparrow {
        out.push(dir.join(".sparrow").join("INSTRUCTIONS.md"));
    }
    out
}

fn scan_instruction_dirs(
    root: &Path,
    dir: &Path,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<InstructionDoc>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_entry(&name) {
            continue;
        }

        if path.is_dir() {
            for candidate in instruction_candidates_in_dir(&path, true) {
                push_doc(root, &candidate, InstructionScope::Directory, seen, docs);
            }
            scan_instruction_dirs(root, &path, seen, docs);
        }
    }
}

fn should_skip_entry(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "node_modules"
            | "dist"
            | "build"
            | ".claude"
            | ".codex-remote-attachments"
    )
}

fn push_doc(
    root: &Path,
    path: &Path,
    scope: InstructionScope,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<InstructionDoc>,
) {
    if !path.is_file() {
        return;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical.clone()) {
        return;
    }
    let mut content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return,
    };
    if content.len() > MAX_INSTRUCTION_FILE_BYTES {
        content.truncate(MAX_INSTRUCTION_FILE_BYTES);
        content.push_str("\n\n[truncated by Sparrow instruction loader]");
    }
    let relative_path = canonical
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| canonical.to_string_lossy().to_string())
        .replace('\\', "/");
    let depth = relative_path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .count();
    docs.push(InstructionDoc {
        scope,
        path: canonical,
        relative_path,
        depth,
        content,
    });
}

fn sort_instruction_docs(root: &Path, task: &str, docs: &mut [InstructionDoc]) {
    let task_lower = task.to_lowercase();
    docs.sort_by(|a, b| {
        let a_relevant = instruction_relevance(root, a, &task_lower);
        let b_relevant = instruction_relevance(root, b, &task_lower);
        a.scope_rank()
            .cmp(&b.scope_rank())
            .then_with(|| b_relevant.cmp(&a_relevant))
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
}

fn instruction_relevance(root: &Path, doc: &InstructionDoc, task_lower: &str) -> bool {
    let rel = doc.relative_path.to_lowercase();
    if rel != doc.path.to_string_lossy().to_lowercase() && task_lower.contains(&rel) {
        return true;
    }
    let parent = doc.path.parent().unwrap_or(root);
    if let Ok(parent_rel) = parent.strip_prefix(root) {
        let parent_rel = parent_rel.to_string_lossy().to_lowercase();
        return !parent_rel.is_empty() && task_lower.contains(&parent_rel);
    }
    false
}

impl InstructionDoc {
    fn scope_rank(&self) -> u8 {
        match self.scope {
            InstructionScope::User => 0,
            InstructionScope::Workspace => 1,
            InstructionScope::Directory => 2,
        }
    }
}

fn enforce_total_limit(docs: Vec<InstructionDoc>) -> Vec<InstructionDoc> {
    let mut total = 0usize;
    let mut out = Vec::new();
    for mut doc in docs {
        if total >= MAX_TOTAL_INSTRUCTION_BYTES {
            break;
        }
        let remaining = MAX_TOTAL_INSTRUCTION_BYTES - total;
        if doc.content.len() > remaining {
            doc.content.truncate(remaining);
            doc.content
                .push_str("\n\n[truncated by Sparrow instruction loader]");
        }
        total += doc.content.len();
        out.push(doc);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sparrow-instructions-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovers_workspace_and_nested_instruction_files() {
        let root = temp_dir("nested");
        std::fs::write(root.join("AGENTS.md"), "root agents").unwrap();
        std::fs::create_dir_all(root.join("src/.sparrow")).unwrap();
        std::fs::write(
            root.join("src/.sparrow/INSTRUCTIONS.md"),
            "src instructions",
        )
        .unwrap();
        std::fs::write(root.join("src/CLAUDE.md"), "src claude").unwrap();

        let docs = discover_workspace_instructions(&root, "edit src/main.rs");
        let rels: Vec<_> = docs.iter().map(|doc| doc.relative_path.as_str()).collect();
        assert!(rels.contains(&"AGENTS.md"));
        assert!(rels.contains(&"src/.sparrow/INSTRUCTIONS.md"));
        assert!(rels.contains(&"src/CLAUDE.md"));
        assert!(
            docs.iter()
                .any(|doc| doc.content.contains("src instructions"))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skips_target_and_node_modules() {
        let root = temp_dir("skip");
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("target/AGENTS.md"), "nope").unwrap();
        std::fs::write(root.join("node_modules/pkg/CLAUDE.md"), "nope").unwrap();

        let docs = discover_workspace_instructions(&root, "");
        assert!(
            docs.iter()
                .all(|doc| !doc.content.contains("nope") && !doc.relative_path.contains("target"))
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
