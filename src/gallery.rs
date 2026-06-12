//! v0.9 Pilier 6 — la galerie des possibles.
//!
//! On ne pousse pas les gens à *avoir* Sparrow, on leur montre tout ce qu'ils
//! peuvent *faire* avec. `sparrow idees` (alias `ideas`) présente une galerie
//! de recettes concrètes, classées par personne plutôt que par fonctionnalité.
//! Chaque recette EST le tutoriel : son `prompt` est prêt à coller.

use crate::humanize::Lang;

/// Who a recipe is for — the personas from PLAN_v0.9.0 §1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persona {
    Enfant,
    Enseignant,
    GrandMere,
    Artisan,
    Maison,
    Builder,
    Developpeur,
    Expert,
}

impl Persona {
    /// Stable lowercase slug used for filtering on the CLI.
    pub fn slug(&self) -> &'static str {
        match self {
            Persona::Enfant => "enfant",
            Persona::Enseignant => "enseignant",
            Persona::GrandMere => "grand-mere",
            Persona::Artisan => "artisan",
            Persona::Maison => "maison",
            Persona::Builder => "builder",
            Persona::Developpeur => "developpeur",
            Persona::Expert => "expert",
        }
    }

    pub fn label(&self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Persona::Enfant, Lang::Fr) => "Pour un enfant",
            (Persona::Enfant, Lang::En) => "For a child",
            (Persona::Enseignant, Lang::Fr) => "Pour un enseignant",
            (Persona::Enseignant, Lang::En) => "For a teacher",
            (Persona::GrandMere, Lang::Fr) => "Pour une grand-mère",
            (Persona::GrandMere, Lang::En) => "For a grandparent",
            (Persona::Artisan, Lang::Fr) => "Pour un artisan",
            (Persona::Artisan, Lang::En) => "For a tradesperson",
            (Persona::Maison, Lang::Fr) => "À la maison",
            (Persona::Maison, Lang::En) => "At home",
            (Persona::Builder, Lang::Fr) => "Pour un créateur",
            (Persona::Builder, Lang::En) => "For a builder",
            (Persona::Developpeur, Lang::Fr) => "Pour un développeur",
            (Persona::Developpeur, Lang::En) => "For a developer",
            (Persona::Expert, Lang::Fr) => "Pour un expert IA",
            (Persona::Expert, Lang::En) => "For an AI expert",
        }
    }

    fn from_slug(s: &str) -> Option<Persona> {
        let s = s.trim().to_lowercase();
        [
            Persona::Enfant,
            Persona::Enseignant,
            Persona::GrandMere,
            Persona::Artisan,
            Persona::Maison,
            Persona::Builder,
            Persona::Developpeur,
            Persona::Expert,
        ]
        .into_iter()
        .find(|p| p.slug() == s || p.slug().starts_with(&s))
    }
}

/// One ready-to-run recipe.
pub struct Recipe {
    pub persona: Persona,
    pub title_fr: &'static str,
    pub title_en: &'static str,
    /// The exact prompt the user can run — this is the tutorial.
    pub prompt_fr: &'static str,
    pub prompt_en: &'static str,
    pub est: &'static str,
}

impl Recipe {
    pub fn title(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Fr => self.title_fr,
            Lang::En => self.title_en,
        }
    }
    pub fn prompt(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Fr => self.prompt_fr,
            Lang::En => self.prompt_en,
        }
    }
}

const RECIPES: &[Recipe] = &[
    Recipe {
        persona: Persona::Enseignant,
        title_fr: "30 quiz à difficulté progressive depuis mon cours",
        title_en: "30 progressive quizzes from my lesson",
        prompt_fr: "Transforme le fichier de mon cours en 30 questions de quiz à difficulté croissante, avec les réponses à la fin.",
        prompt_en: "Turn my lesson file into 30 quiz questions of increasing difficulty, with answers at the end.",
        est: "~2 min",
    },
    Recipe {
        persona: Persona::GrandMere,
        title_fr: "Ranger mes photos par année et par personne",
        title_en: "Sort my photos by year and by person",
        prompt_fr: "Range les photos de ce dossier dans des sous-dossiers par année, et explique-moi chaque étape simplement.",
        prompt_en: "Sort the photos in this folder into subfolders by year, and explain each step to me simply.",
        est: "~5 min",
    },
    Recipe {
        persona: Persona::Artisan,
        title_fr: "Croiser mes factures Excel et mes bons de commande",
        title_en: "Cross-check my Excel invoices against purchase orders",
        prompt_fr: "Compare mon fichier de factures et mon fichier de bons de commande, et liste-moi tous les écarts de montant.",
        prompt_en: "Compare my invoices file and my purchase-orders file, and list every amount mismatch.",
        est: "~3 min",
    },
    Recipe {
        persona: Persona::Enfant,
        title_fr: "Pourquoi mon jeu Scratch plante ?",
        title_en: "Why does my Scratch game crash?",
        prompt_fr: "Explique-moi pourquoi mon projet plante, comme si j'avais 9 ans, et aide-moi à le réparer.",
        prompt_en: "Explain why my project crashes, like I'm 9 years old, and help me fix it.",
        est: "~2 min",
    },
    Recipe {
        persona: Persona::Maison,
        title_fr: "Un planning de repas de la semaine",
        title_en: "A weekly meal plan",
        prompt_fr: "Fais-moi un planning de repas équilibrés pour la semaine à partir de ma liste de courses, et la liste de ce qui manque.",
        prompt_en: "Make me a balanced weekly meal plan from my shopping list, plus a list of what's missing.",
        est: "~2 min",
    },
    Recipe {
        persona: Persona::Builder,
        title_fr: "Une page de présentation pour mon idée",
        title_en: "A landing page for my idea",
        prompt_fr: "Monte-moi une page web simple et jolie qui présente mon idée, prête à ouvrir dans un navigateur.",
        prompt_en: "Build me a simple, good-looking web page that presents my idea, ready to open in a browser.",
        est: "~10 min",
    },
    Recipe {
        persona: Persona::Developpeur,
        title_fr: "Trouver pourquoi ce test échoue 1 fois sur 20",
        title_en: "Find why this test flakes 1 in 20 runs",
        prompt_fr: "Ce test échoue de façon intermittente. Trouve la cause de l'instabilité et propose un correctif.",
        prompt_en: "This test fails intermittently. Find the source of the flakiness and propose a fix.",
        est: "~5 min",
    },
    Recipe {
        persona: Persona::Developpeur,
        title_fr: "Expliquer ce dépôt que je découvre",
        title_en: "Explain this repo I'm new to",
        prompt_fr: "Fais-moi le tour de ce dépôt : à quoi il sert, son architecture, et par où commencer pour contribuer.",
        prompt_en: "Give me a tour of this repo: what it does, its architecture, and where to start contributing.",
        est: "~3 min",
    },
    Recipe {
        persona: Persona::Expert,
        title_fr: "Orchestrer un swarm sur un gros monorepo",
        title_en: "Orchestrate a swarm over a large monorepo",
        prompt_fr: "Lance un swarm : un planner qui découpe la tâche, des coders en parallèle par module, un verifier qui teste chaque diff.",
        prompt_en: "Run a swarm: a planner that splits the task, coders in parallel per module, a verifier that tests each diff.",
        est: "variable",
    },
];

/// All recipes, optionally filtered by a persona slug and/or a free-text query
/// (matched against title and prompt, case-insensitive).
pub fn search<'a>(persona: Option<&str>, query: Option<&str>) -> Vec<&'a Recipe> {
    let persona = persona.and_then(Persona::from_slug);
    let q = query
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    RECIPES
        .iter()
        .filter(|r| persona.is_none_or(|p| r.persona == p))
        .filter(|r| {
            q.as_ref().is_none_or(|q| {
                r.title_fr.to_lowercase().contains(q)
                    || r.title_en.to_lowercase().contains(q)
                    || r.prompt_fr.to_lowercase().contains(q)
                    || r.prompt_en.to_lowercase().contains(q)
            })
        })
        .collect()
}

/// Distinct persona slugs that have at least one recipe, in display order.
pub fn personas() -> Vec<&'static str> {
    let mut seen = Vec::new();
    for r in RECIPES {
        if !seen.contains(&r.persona.slug()) {
            seen.push(r.persona.slug());
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_everything_with_no_filter() {
        assert_eq!(search(None, None).len(), RECIPES.len());
        assert!(RECIPES.len() >= 8, "ship a meaningful starter gallery");
    }

    #[test]
    fn filters_by_persona() {
        let dev = search(Some("developpeur"), None);
        assert!(!dev.is_empty());
        assert!(dev.iter().all(|r| r.persona == Persona::Developpeur));
        // Prefix match works too.
        assert_eq!(search(Some("dev"), None).len(), dev.len());
    }

    #[test]
    fn full_text_search_matches_title_and_prompt() {
        assert!(!search(None, Some("photos")).is_empty());
        assert!(!search(None, Some("swarm")).is_empty());
        assert!(search(None, Some("zzzznotfound")).is_empty());
    }

    #[test]
    fn personas_list_is_non_empty_and_unique() {
        let p = personas();
        assert!(!p.is_empty());
        let mut sorted = p.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), p.len(), "no duplicate personas");
    }

    #[test]
    fn every_recipe_has_both_languages() {
        for r in RECIPES {
            assert!(!r.prompt(Lang::Fr).is_empty());
            assert!(!r.prompt(Lang::En).is_empty());
            assert!(!r.title(Lang::Fr).is_empty());
            assert!(!r.title(Lang::En).is_empty());
        }
    }
}
