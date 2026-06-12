//! v0.9 Pilier 1 — l'accueil chaleureux et la détection de contexte.
//!
//! `sparrow bonjour` (alias `hello`, `salut`) ouvre une porte d'entrée
//! accueillante : « Qu'est-ce qu'on règle aujourd'hui ? », une suggestion
//! adaptée à ce qu'il y a dans le dossier courant, et quelques idées concrètes.
//! C'est le premier contact « waouh », sans jargon.

use crate::humanize::Lang;
use std::path::Path;

/// What Sparrow noticed about the current directory, used to lead with the
/// most relevant offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// A git repository with a merge/rebase in progress.
    GitConflict,
    /// A Node project whose dependencies aren't installed yet.
    NodeNeedsInstall,
    /// A folder that's mostly images.
    ManyImages,
    /// A git repository (no obvious problem).
    GitRepo,
    /// Nothing specific.
    Generic,
}

/// Inspect `cwd` cheaply (a few stat calls, no recursion beyond one level) to
/// pick the most relevant opening offer.
pub fn detect_context(cwd: &Path) -> Context {
    let git = cwd.join(".git");
    if git.is_dir() {
        // Merge/rebase in progress → a conflict the user likely wants resolved.
        if git.join("MERGE_HEAD").exists()
            || git.join("rebase-merge").exists()
            || git.join("rebase-apply").exists()
        {
            return Context::GitConflict;
        }
    }
    if cwd.join("package.json").is_file() && !cwd.join("node_modules").exists() {
        return Context::NodeNeedsInstall;
    }
    if looks_like_photo_folder(cwd) {
        return Context::ManyImages;
    }
    if git.is_dir() {
        return Context::GitRepo;
    }
    Context::Generic
}

/// True when the directory's top level is dominated by image files.
fn looks_like_photo_folder(cwd: &Path) -> bool {
    let exts = ["jpg", "jpeg", "png", "heic", "gif", "webp"];
    let mut images = 0usize;
    let mut total = 0usize;
    if let Ok(entries) = std::fs::read_dir(cwd) {
        for e in entries.flatten().take(200) {
            if e.path().is_file() {
                total += 1;
                if let Some(ext) = e.path().extension().and_then(|x| x.to_str()) {
                    if exts.contains(&ext.to_lowercase().as_str()) {
                        images += 1;
                    }
                }
            }
        }
    }
    total >= 8 && images * 2 >= total
}

/// The contextual one-liner offer for a detected context.
fn context_offer(ctx: Context, lang: Lang) -> Option<String> {
    let s = match (ctx, lang) {
        (Context::GitConflict, Lang::Fr) => {
            "Je vois un conflit git en cours ici. Tu veux que je t'aide à le régler ? → sparrow fix"
        }
        (Context::GitConflict, Lang::En) => {
            "I see a git conflict in progress here. Want me to help resolve it? → sparrow fix"
        }
        (Context::NodeNeedsInstall, Lang::Fr) => {
            "Ce projet n'est pas installé (pas de node_modules). Je m'en occupe ? → sparrow fix"
        }
        (Context::NodeNeedsInstall, Lang::En) => {
            "This project isn't installed (no node_modules). Want me to handle it? → sparrow fix"
        }
        (Context::ManyImages, Lang::Fr) => {
            "Beaucoup de photos ici. Je peux les ranger par année. → sparrow idees grand-mere"
        }
        (Context::ManyImages, Lang::En) => {
            "Lots of photos here. I can sort them by year. → sparrow ideas grandparent"
        }
        (Context::GitRepo, Lang::Fr) => {
            "Tu es dans un projet de code. Un bug à corriger ? → sparrow fix · Comprendre le code ? → sparrow explique"
        }
        (Context::GitRepo, Lang::En) => {
            "You're in a code project. A bug to fix? → sparrow fix · Understand the code? → sparrow explain"
        }
        (Context::Generic, _) => return None,
    };
    Some(s.to_string())
}

/// Build the full warm welcome for the current directory.
pub fn welcome_text(cwd: &Path, lang: Lang) -> String {
    let mut out = String::new();
    let header = match lang {
        Lang::Fr => "🐦  Salut ! Qu'est-ce qu'on règle aujourd'hui ?",
        Lang::En => "🐦  Hi! What are we sorting out today?",
    };
    out.push_str(header);
    out.push_str("\n\n");

    if let Some(offer) = context_offer(detect_context(cwd), lang) {
        out.push_str("    ");
        out.push_str(&offer);
        out.push_str("\n\n");
    }

    let body = match lang {
        Lang::Fr => {
            "    Décris ton problème avec tes mots :\n\
             \x20     · sparrow fix \"mon site ne s'affiche plus\"\n\
             \x20     · sparrow explique \"ce message d'erreur\"\n\
             \x20     · sparrow idees      (tout ce que tu peux faire)\n\
             \x20     · sparrow whatis token   (c'est quoi ce mot ?)\n\n\
            Et si ça ne te plaît pas : sparrow annule remet tout comme avant."
        }
        Lang::En => {
            "    Describe your problem in your own words:\n\
             \x20     · sparrow fix \"my site won't load\"\n\
             \x20     · sparrow explain \"this error message\"\n\
             \x20     · sparrow ideas      (everything you can do)\n\
             \x20     · sparrow whatis token   (what's this word?)\n\n\
            And if you don't like it: sparrow undo puts everything back."
        }
    };
    out.push_str(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "sparrow-welcome-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn detects_node_needs_install() {
        let d = tmp("node");
        fs::write(d.join("package.json"), "{}").unwrap();
        assert_eq!(detect_context(&d), Context::NodeNeedsInstall);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn detects_git_conflict() {
        let d = tmp("conflict");
        fs::create_dir_all(d.join(".git")).unwrap();
        fs::write(d.join(".git").join("MERGE_HEAD"), "x").unwrap();
        assert_eq!(detect_context(&d), Context::GitConflict);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn detects_photo_folder() {
        let d = tmp("photos");
        for i in 0..10 {
            fs::write(d.join(format!("img{i}.jpg")), "x").unwrap();
        }
        assert_eq!(detect_context(&d), Context::ManyImages);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn empty_dir_is_generic() {
        let d = tmp("empty");
        assert_eq!(detect_context(&d), Context::Generic);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn welcome_text_is_warm_and_jargon_free() {
        let d = tmp("welcome");
        let text = welcome_text(&d, Lang::Fr);
        assert!(text.contains("Qu'est-ce qu'on règle"));
        assert!(text.contains("sparrow fix"));
        assert!(text.contains("sparrow annule"));
        // No raw technical noise in the welcome itself.
        for bad in ["tier", "tokens", "0.0.0.0", "[Tool"] {
            assert!(!text.contains(bad), "welcome leaked `{bad}`");
        }
        let _ = fs::remove_dir_all(d);
    }
}
