use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::UNIX_EPOCH;

use tree_sitter::{Language, Node, Parser, Query, QueryCursor};

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist", "build", ".venv"];
const MAX_FILES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Fn,
    Struct,
    Enum,
    Trait,
    Impl,
    Class,
    Method,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Class => "class",
            Self::Method => "method",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolIndex {
    pub defs: HashMap<String, Vec<SymbolDef>>,
}

#[derive(Debug, Clone)]
struct CachedIndex {
    hash: u64,
    index: Arc<SymbolIndex>,
}

static CACHE: OnceLock<RwLock<HashMap<PathBuf, CachedIndex>>> = OnceLock::new();

impl SymbolIndex {
    pub fn build(root: &Path) -> SymbolIndex {
        let root = normalize_root(root);
        let mut files = Vec::new();
        walk_code_files(&root, &mut files);
        let hash = content_hash(&files);
        let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));

        if let Ok(guard) = cache.read() {
            if let Some(cached) = guard.get(&root) {
                if cached.hash == hash {
                    return (*cached.index).clone();
                }
            }
        }

        let mut index = SymbolIndex::default();
        for file in files {
            if let Ok(content) = std::fs::read_to_string(&file) {
                for def in parse_file(&root, &file, &content) {
                    index.defs.entry(def.name.clone()).or_default().push(def);
                }
            }
        }
        for defs in index.defs.values_mut() {
            defs.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        }

        if let Ok(mut guard) = cache.write() {
            guard.insert(
                root,
                CachedIndex {
                    hash,
                    index: Arc::new(index.clone()),
                },
            );
        }
        index
    }

    pub fn find_definition(&self, name: &str) -> Vec<&SymbolDef> {
        self.defs
            .get(name)
            .map(|defs| defs.iter().collect())
            .unwrap_or_default()
    }

    pub fn outline(&self, file: &Path) -> Vec<&SymbolDef> {
        let normalized = normalize_separators(file);
        let mut out: Vec<&SymbolDef> = self
            .defs
            .values()
            .flat_map(|defs| defs.iter())
            .filter(|def| normalize_separators(&def.file) == normalized)
            .collect();
        out.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
        out
    }
}

fn normalize_root(root: &Path) -> PathBuf {
    root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
}

fn normalize_separators(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn walk_code_files(root: &Path, out: &mut Vec<PathBuf>) {
    let repo = git2::Repository::discover(root).ok();
    walk_code_files_inner(root, &repo, out);
}

fn walk_code_files_inner(dir: &Path, repo: &Option<git2::Repository>, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_ignored(repo, &path) {
            continue;
        }
        if path.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk_code_files_inner(&path, repo, out);
        } else if language_for_path(&path).is_some() {
            out.push(path);
        }
    }
}

fn is_ignored(repo: &Option<git2::Repository>, path: &Path) -> bool {
    repo.as_ref()
        .and_then(|repo| repo.status_should_ignore(path).ok())
        .unwrap_or(false)
}

fn content_hash(files: &[PathBuf]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for file in files {
        hash = fnv_mix(hash, normalize_separators(file).as_bytes());
        if let Ok(meta) = file.metadata() {
            hash = fnv_mix(hash, &meta.len().to_le_bytes());
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            hash = fnv_mix(hash, &modified.to_le_bytes());
        }
    }
    hash
}

fn fnv_mix(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn language_for_path(path: &Path) -> Option<(&'static str, Language)> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Some(("rust", tree_sitter_rust::LANGUAGE.into())),
        Some("py") => Some(("python", tree_sitter_python::LANGUAGE.into())),
        Some("js") | Some("jsx") => Some(("javascript", tree_sitter_javascript::LANGUAGE.into())),
        Some("ts") | Some("tsx") => Some(("typescript", tree_sitter_javascript::LANGUAGE.into())),
        _ => None,
    }
}

fn query_for_language(language_name: &str, language: &Language) -> Option<Query> {
    let source = match language_name {
        "rust" => {
            r#"
            (function_item name: (identifier) @name) @fn
            (struct_item name: (type_identifier) @name) @struct
            (enum_item name: (type_identifier) @name) @enum
            (trait_item name: (type_identifier) @name) @trait
            (impl_item type: (_) @name) @impl
            "#
        }
        "python" => {
            r#"
            (function_definition name: (identifier) @name) @fn
            (class_definition name: (identifier) @name) @class
            "#
        }
        "javascript" | "typescript" => {
            r#"
            (function_declaration name: (identifier) @name) @fn
            (method_definition name: (property_identifier) @name) @method
            (class_declaration name: (identifier) @name) @class
            (lexical_declaration
              (variable_declarator
                name: (identifier) @name
                value: [(arrow_function) (function_expression)])) @fn
            "#
        }
        _ => return None,
    };
    Query::new(language, source).ok()
}

fn parse_file(root: &Path, file: &Path, content: &str) -> Vec<SymbolDef> {
    let Some((language_name, language)) = language_for_path(file) else {
        return Vec::new();
    };
    let Some(query) = query_for_language(language_name, &language) else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(content, None) else {
        return Vec::new();
    };
    let bytes = content.as_bytes();
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), bytes);
    let mut defs = Vec::new();

    for mat in matches {
        let mut name_node = None;
        let mut def_node = None;
        let mut kind = None;
        for capture in mat.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            if capture_name == "name" {
                name_node = Some(capture.node);
            } else {
                def_node = Some(capture.node);
                kind = kind_from_capture(capture_name);
            }
        }
        let Some(name_node) = name_node else {
            continue;
        };
        let Some(kind) = kind else {
            continue;
        };
        let node = def_node.unwrap_or(name_node);
        let name = node_text(name_node, bytes);
        if name.is_empty() {
            continue;
        }
        let rel = file.strip_prefix(root).unwrap_or(file).to_path_buf();
        defs.push(SymbolDef {
            name: clean_symbol_name(&name),
            kind,
            file: rel,
            line: node.start_position().row + 1,
            signature: signature_for_node(node, content),
        });
    }

    defs
}

fn kind_from_capture(capture: &str) -> Option<SymbolKind> {
    match capture {
        "fn" => Some(SymbolKind::Fn),
        "struct" => Some(SymbolKind::Struct),
        "enum" => Some(SymbolKind::Enum),
        "trait" => Some(SymbolKind::Trait),
        "impl" => Some(SymbolKind::Impl),
        "class" => Some(SymbolKind::Class),
        "method" => Some(SymbolKind::Method),
        _ => None,
    }
}

fn node_text(node: Node<'_>, bytes: &[u8]) -> String {
    node.utf8_text(bytes).unwrap_or("").trim().to_string()
}

fn clean_symbol_name(name: &str) -> String {
    name.split_whitespace()
        .last()
        .unwrap_or(name)
        .trim_matches(|c: char| c == '{' || c == '(' || c == ')' || c == ';')
        .to_string()
}

fn signature_for_node(node: Node<'_>, content: &str) -> String {
    let start = node.start_byte().min(content.len());
    let end = node.end_byte().min(content.len());
    let raw = &content[start..end];
    raw.lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches('{')
        .trim()
        .to_string()
}
