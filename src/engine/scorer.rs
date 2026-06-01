use std::collections::HashMap;

/// TF-IDF style file scorer for intelligent context prioritization.
/// Phase 3 Item 12: scores files by relevance to the task.
pub struct FileScorer;

impl FileScorer {
    /// Score a set of files against a task description.
    /// Returns (file_path, score) sorted by relevance descending.
    pub fn score_files(task: &str, files: &[crate::memory::FileEntry], symbols: &[crate::memory::SymbolEntry]) -> Vec<(String, f64)> {
        let task_words: Vec<String> = tokenize(task);
        if task_words.is_empty() {
            return files.iter().map(|f| (f.path.clone(), 0.0)).collect();
        }

        let mut scores: HashMap<String, f64> = HashMap::new();

        // Score by file path matching
        for file in files {
            let path_words = tokenize(&file.path);
            let score = cosine_similarity_words(&task_words, &path_words);
            *scores.entry(file.path.clone()).or_insert(0.0) += score * 2.0;
        }

        // Score by symbol name matching
        for sym in symbols {
            let sym_words = tokenize(&sym.name);
            let score = cosine_similarity_words(&task_words, &sym_words);
            *scores.entry(sym.file.clone()).or_insert(0.0) += score * 3.0;
        }

        let mut result: Vec<(String, f64)> = scores.into_iter().collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Return top N files, always including explicitly mentioned files
    pub fn top_files(task: &str, files: &[crate::memory::FileEntry], symbols: &[crate::memory::SymbolEntry], max: usize) -> Vec<String> {
        let mut scored = Self::score_files(task, files, symbols);

        // Always include files explicitly mentioned in the task
        let task_lower = task.to_lowercase();
        for file in files {
            if task_lower.contains(&file.path.to_lowercase()) {
                if !scored.iter().any(|(p, _)| p == &file.path) {
                    scored.push((file.path.clone(), 10.0));
                }
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max);
        scored.into_iter().map(|(p, _)| p).collect()
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '/')
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Cosine similarity between two word frequency vectors
fn cosine_similarity_words(a: &[String], b: &[String]) -> f64 {
    let mut freq_a: HashMap<&str, f64> = HashMap::new();
    let mut freq_b: HashMap<&str, f64> = HashMap::new();

    for w in a { *freq_a.entry(w.as_str()).or_insert(0.0) += 1.0; }
    for w in b { *freq_b.entry(w.as_str()).or_insert(0.0) += 1.0; }

    let all_words: std::collections::HashSet<&str> = freq_a.keys().chain(freq_b.keys()).copied().collect();
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for w in &all_words {
        let va = freq_a.get(w).copied().unwrap_or(0.0);
        let vb = freq_b.get(w).copied().unwrap_or(0.0);
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }

    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a.sqrt() * norm_b.sqrt()) }
}

// ─── Embedded Python kernel (persistent) ───────────────────────────────────────

pub const PYTHON_KERNEL: &str = r#"
import sys, json, traceback, io
namespace = {}
_stdout = io.StringIO()
while True:
    line = sys.stdin.readline()
    if not line:
        break
    try:
        req = json.loads(line)
        _stdout = io.StringIO()
        sys.stdout = _stdout
        if req.get('mode') == 'eval':
            result = eval(req['code'], namespace)
        else:
            exec(req['code'], namespace)
            result = None
        sys.stdout = sys.__stdout__
        stdout_val = _stdout.getvalue()
        response = {"id": req['id'], "result": str(result) if result is not None else None, "stdout": stdout_val}
    except Exception as e:
        sys.stdout = sys.__stdout__
        response = {"id": req['id'], "error": traceback.format_exc()}
    print(json.dumps(response), flush=True)
"#;
