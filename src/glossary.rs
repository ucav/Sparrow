//! v0.9 Pilier 2 — le glossaire vivant.
//!
//! When a technical term is unavoidable, `sparrow c'est-quoi <mot>` (alias
//! `whatis`) gives a two-sentence, jargon-free definition — instantly,
//! offline, no model call. The table covers Sparrow's *own* vocabulary (the
//! words it shows on screen), not general knowledge: for anything else,
//! `sparrow explique` runs the engine.

use crate::humanize::Lang;

/// One glossary entry: the canonical term plus its French and English gloss.
struct Entry {
    /// Lowercase aliases that map to this entry (term + common variants).
    keys: &'static [&'static str],
    fr: &'static str,
    en: &'static str,
}

const GLOSSARY: &[Entry] = &[
    Entry {
        keys: &["checkpoint", "point de sauvegarde", "sauvegarde"],
        fr: "Un point de sauvegarde : une photo de tes fichiers prise avant que Sparrow ne les modifie. Tu peux toujours y revenir avec « sparrow annule ».",
        en: "A checkpoint: a snapshot of your files taken before Sparrow changes them. You can always return to it with “sparrow undo”.",
    },
    Entry {
        keys: &["token", "tokens", "jeton", "jetons"],
        fr: "Un token, c'est un petit morceau de texte (environ trois quarts d'un mot). Les modèles d'IA comptent leur travail en tokens, et c'est ce qui détermine le coût.",
        en: "A token is a small piece of text (about three-quarters of a word). AI models measure their work in tokens, and that's what drives the cost.",
    },
    Entry {
        keys: &["swarm", "essaim", "multi-agent", "multi-agents"],
        fr: "Un swarm, c'est plusieurs assistants qui travaillent ensemble : l'un planifie, un autre code, un troisième vérifie. Sparrow s'en sert pour les tâches complexes.",
        en: "A swarm is several assistants working together: one plans, one codes, one verifies. Sparrow uses it for complex tasks.",
    },
    Entry {
        keys: &["mcp", "model context protocol"],
        fr: "MCP est une prise standard qui permet à Sparrow de se brancher à des outils externes (ta messagerie, ta base de données, etc.). Tu n'as pas besoin d'en connaître les détails pour l'utiliser.",
        en: "MCP is a standard plug that lets Sparrow connect to external tools (your mail, your database, etc.). You don't need to know its details to use it.",
    },
    Entry {
        keys: &["provider", "providers", "fournisseur", "fournisseurs"],
        fr: "Un provider, c'est l'entreprise qui héberge un modèle d'IA (par exemple Anthropic, OpenAI, ou un modèle qui tourne sur ton ordinateur). Sparrow peut en utiliser plusieurs.",
        en: "A provider is the company hosting an AI model (e.g. Anthropic, OpenAI, or a model running on your own computer). Sparrow can use several.",
    },
    Entry {
        keys: &["routing", "routage", "route"],
        fr: "Le routage, c'est Sparrow qui choisit tout seul le meilleur modèle pour chaque tâche — le moins cher capable de bien la faire. Tu n'as rien à régler.",
        en: "Routing is Sparrow picking the best model for each task on its own — the cheapest one able to do it well. You don't have to set anything.",
    },
    Entry {
        keys: &["autonomy", "autonomie"],
        fr: "L'autonomie, c'est le degré de liberté que tu donnes à Sparrow : te demander avant chaque action, ou agir seul en te montrant tout. Tu choisis.",
        en: "Autonomy is how much freedom you give Sparrow: ask before every action, or act on its own while showing you everything. Your choice.",
    },
    Entry {
        keys: &["tier", "palier", "niveau"],
        fr: "Le tier (ou palier), c'est la difficulté estimée d'une tâche. Sparrow s'en sert pour envoyer les tâches simples à des modèles légers et les tâches dures à des modèles puissants.",
        en: "The tier is a task's estimated difficulty. Sparrow uses it to send easy tasks to light models and hard tasks to powerful ones.",
    },
    Entry {
        keys: &["agent", "agents"],
        fr: "Un agent, c'est une personnalité d'assistant avec son rôle et sa façon de travailler. Tu peux en créer plusieurs (un pour le code, un pour la rédaction…).",
        en: "An agent is an assistant personality with its own role and way of working. You can create several (one for code, one for writing…).",
    },
    Entry {
        keys: &["skill", "skills", "compétence", "compétences"],
        fr: "Un skill, c'est une recette que Sparrow sait suivre pour un type de tâche (corriger un bug, écrire des tests…). Il en a déjà plusieurs et peut en apprendre.",
        en: "A skill is a recipe Sparrow can follow for a kind of task (fix a bug, write tests…). It ships with several and can learn more.",
    },
    Entry {
        keys: &["rewind", "annule", "annuler", "undo"],
        fr: "Annuler (rewind), c'est remettre tes fichiers exactement comme ils étaient à un point de sauvegarde précédent. Rien n'est jamais perdu pour de bon.",
        en: "Undo (rewind) puts your files back exactly as they were at an earlier checkpoint. Nothing is ever lost for good.",
    },
    Entry {
        keys: &["cli", "ligne de commande", "terminal"],
        fr: "Le CLI, c'est la fenêtre où tu tapes des commandes au clavier. C'est juste une façon de parler à ton ordinateur avec des mots plutôt qu'avec la souris.",
        en: "The CLI is the window where you type commands. It's just a way of talking to your computer with words instead of the mouse.",
    },
    Entry {
        keys: &["budget", "plafond"],
        fr: "Le budget, c'est la limite de dépense que tu fixes. Sparrow s'arrête tout seul avant de la dépasser — tu ne peux pas avoir de mauvaise surprise.",
        en: "The budget is the spending limit you set. Sparrow stops on its own before crossing it — no nasty surprises.",
    },
];

/// Look up a Sparrow term. Case-insensitive, trims surrounding whitespace.
/// Returns the two-sentence gloss in the requested language, or `None` when
/// the term isn't part of Sparrow's vocabulary.
pub fn lookup(term: &str, lang: Lang) -> Option<String> {
    let needle = term.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }
    GLOSSARY
        .iter()
        .find(|e| e.keys.contains(&needle.as_str()))
        .map(|e| match lang {
            Lang::Fr => e.fr.to_string(),
            Lang::En => e.en.to_string(),
        })
}

/// Every distinct canonical term (first key of each entry), for listing.
pub fn terms() -> Vec<&'static str> {
    GLOSSARY
        .iter()
        .flat_map(|e| e.keys.first().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_known_terms_case_insensitively() {
        assert!(
            lookup("Checkpoint", Lang::Fr)
                .unwrap()
                .contains("photo de tes fichiers")
        );
        assert!(lookup("TOKEN", Lang::En).unwrap().contains("piece of text"));
        assert!(lookup("  swarm  ", Lang::Fr).is_some());
    }

    #[test]
    fn french_aliases_resolve() {
        assert!(lookup("point de sauvegarde", Lang::Fr).is_some());
        assert!(lookup("routage", Lang::Fr).is_some());
        assert!(lookup("fournisseur", Lang::Fr).is_some());
    }

    #[test]
    fn unknown_term_returns_none() {
        assert!(lookup("quantum entanglement", Lang::Fr).is_none());
        assert!(lookup("", Lang::Fr).is_none());
    }

    #[test]
    fn definitions_are_short_and_jargon_light() {
        // Each gloss should be a couple of sentences, not a wall of text.
        for t in terms() {
            let def = lookup(t, Lang::Fr).unwrap();
            assert!(def.len() < 400, "gloss for `{t}` is too long");
            assert!(def.contains('.'), "gloss for `{t}` should be sentences");
        }
    }
}
