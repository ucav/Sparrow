//! Humanized error messages in French.
//!
//! Maps technical errors (HTTP status codes, connection failures, model lookup
//! failures, config errors) to friendly, actionable messages in French. Every
//! public function returns a `String` suitable for direct display to the user.

use crate::provider::BrainError;

/// Translate a [`BrainError`] into a French human-readable message.
///
/// The `provider_label` is the display name of the provider (e.g. "Anthropic",
/// "NVIDIA NIM"). The `model` is the model name that was requested when the
/// error occurred (empty string when unknown).
///
/// # Example
/// ```ignore
/// let err = BrainError::ServerError { status: 401, body: String::new() };
/// let msg = errors::humanize_brain_error(&err, "Anthropic", "claude-sonnet-4-6");
/// println!("{}", msg);
/// ```
pub fn humanize_brain_error(err: &BrainError, provider_label: &str, model: &str) -> String {
    match err {
        BrainError::RateLimit { retry_after } => {
            humanize_rate_limit(provider_label, *retry_after)
        }
        BrainError::ServerError { status, body } => {
            humanize_http_error(*status, body, provider_label, model)
        }
        BrainError::Timeout => {
            format!(
                "⏱️  {provider} a mis trop de temps à répondre. Réessaie ou change de provider (sparrow route set <provider>).",
                provider = provider_label,
            )
        }
        BrainError::Refusal(msg) => {
            format!(
                "🚫 {provider} a refusé la requête : {msg}\n→ Reformule ta demande ou change de modèle avec sparrow model --set <provider>:<model>.",
                provider = provider_label,
                msg = msg,
            )
        }
        BrainError::Unknown(msg) => {
            format!(
                "❓ Erreur inconnue chez {provider} : {msg}\n→ Lance sparrow doctor pour un diagnostic complet.",
                provider = provider_label,
                msg = msg,
            )
        }
    }
}

/// Translate a raw HTTP status code + body into a French message.
fn humanize_http_error(status: u16, body: &str, provider_label: &str, model: &str) -> String {
    match status {
        401 | 403 => humanize_unauthorized(provider_label),
        404 => {
            if body.contains("model") || body.contains("Model") {
                humanize_model_not_found(provider_label, model)
            } else {
                format!(
                    "🔍 Ressource introuvable chez {provider}. Vérifie l'URL de base (sparrow config --edit).",
                    provider = provider_label,
                )
            }
        }
        429 => humanize_rate_limit(provider_label, None),
        500..=599 => {
            if body.to_lowercase().contains("overloaded")
                || body.to_lowercase().contains("capacity")
            {
                format!(
                    "🏋️ {provider} est surchargé. Réessaie dans quelques minutes ou change de provider (sparrow route set <provider>).",
                    provider = provider_label,
                )
            } else {
                format!(
                    "💥 Erreur serveur {provider} (HTTP {status}). C'est probablement temporaire — réessaie dans 30 secondes.\n→ Détails : {body}",
                    provider = provider_label,
                    status = status,
                    body = body,
                )
            }
        }
        _ => format!(
            "⚠️  Erreur HTTP {status} chez {provider} : {body}\n→ Lance sparrow doctor pour un diagnostic.",
            status = status,
            provider = provider_label,
            body = body,
        ),
    }
}

/// "Ta clé API est invalide ou expirée…"
fn humanize_unauthorized(provider_label: &str) -> String {
    let (url, signup_hint) = match provider_label.to_lowercase().as_str() {
        "anthropic" => (
            "https://console.anthropic.com/settings/keys",
            " (payant, ~20$/mois de crédits offerts)",
        ),
        "nvidia nim" | "nvidia" => (
            "https://build.nvidia.com/explore/discover",
            " (gratuit — crée un compte NVIDIA Developer)",
        ),
        "openai" | "openai codex" => (
            "https://platform.openai.com/api-keys",
            " (payant, crédits gratuits à l'inscription)",
        ),
        "google gemini" | "gemini" => (
            "https://aistudio.google.com/app/apikey",
            " (gratuit jusqu'à 1500 requêtes/jour)",
        ),
        "groq" | "groq" => (
            "https://console.groq.com/keys",
            " (gratuit — généreux tier gratuit)",
        ),
        "deepseek" => (
            "https://platform.deepseek.com/api_keys",
            " (très bon marché, ~0.27$/M tokens)",
        ),
        "openrouter" => (
            "https://openrouter.ai/keys",
            " (crédits gratuits à l'inscription)",
        ),
        "xai" | "xai (grok)" => (
            "https://console.x.ai/",
            " (payant)",
        ),
        _ => ("", ""),
    };

    if url.is_empty() {
        format!(
            "🔑 Ta clé API pour {provider} est invalide ou expirée.\n\
             → Vérifie ta clé avec : sparrow auth list\n\
             → Ajoutes-en une nouvelle : sparrow auth add {provider_lower}",
            provider = provider_label,
            provider_lower = provider_label.to_lowercase(),
        )
    } else {
        format!(
            "🔑 Ta clé API {provider} est invalide ou expirée.\n\
             → Va sur {url} pour en créer une{signup}\n\
             → Puis ajoute-la avec : sparrow auth add {provider_lower}\n\
             → Ou exporte-la : export {env_var}=\"ta-clé\"",
            provider = provider_label,
            url = url,
            signup = signup_hint,
            provider_lower = provider_label.to_lowercase().replace(' ', "-"),
            env_var = provider_env_var(provider_label),
        )
    }
}

/// "T'as envoyé trop de requêtes…"
fn humanize_rate_limit(provider_label: &str, retry_after: Option<u64>) -> String {
    let wait = match retry_after {
        Some(s) if s > 0 => format!("{s} secondes"),
        _ => "quelques minutes".to_string(),
    };
    format!(
        "⏳ T'as envoyé trop de requêtes à {provider}. Réessaie dans {wait}.\n\
         → Astuce : utilise un modèle moins cher pour les tâches simples (sparrow route set nvidia).",
        provider = provider_label,
        wait = wait,
    )
}

/// "Le modèle X n'existe pas chez Y…"
fn humanize_model_not_found(provider_label: &str, model: &str) -> String {
    format!(
        "🤷 Le modèle \"{model}\" n'existe pas chez {provider}.\n\
         → Liste les modèles dispos : sparrow model --list\n\
         → Change de modèle : sparrow model --set {provider_lower}:<model>",
        model = model,
        provider = provider_label,
        provider_lower = provider_label.to_lowercase().replace(' ', "-"),
    )
}

/// Guess the standard env-var name for a provider.
fn provider_env_var(provider_label: &str) -> String {
    match provider_label.to_lowercase().as_str() {
        "anthropic" => "ANTHROPIC_API_KEY".into(),
        "nvidia nim" | "nvidia" => "NVIDIA_API_KEY".into(),
        "openai" | "openai codex" => "OPENAI_API_KEY".into(),
        "google gemini" | "gemini" => "GEMINI_API_KEY".into(),
        "groq" => "GROQ_API_KEY".into(),
        "deepseek" => "DEEPSEEK_API_KEY".into(),
        "openrouter" => "OPENROUTER_API_KEY".into(),
        "xai" | "xai (grok)" => "XAI_API_KEY".into(),
        "huggingface" | "hugging face" => "HF_TOKEN".into(),
        "nous" | "nous portal" => "NOUS_API_KEY".into(),
        "novita" | "novitaai" => "NOVITA_API_KEY".into(),
        "alibaba" | "alibaba cloud" => "DASHSCOPE_API_KEY".into(),
        other => format!("{}_API_KEY", other.to_uppercase().replace(' ', "_").replace('-', "_")),
    }
}

/// Translate a connection-refused / DNS / timeout error into a French message.
///
/// Call this when `reqwest` returns a connection error (not an HTTP error).
pub fn humanize_connection_error(provider_label: &str, error_msg: &str) -> String {
    let lower = error_msg.to_lowercase();

    if lower.contains("connection refused") || lower.contains("connect refused") {
        format!(
            "🔌 Impossible de contacter {provider}. Le serveur a refusé la connexion.\n\
             → Vérifie l'URL de base : sparrow config --edit\n\
             → Si t'utilises Ollama : ollama serve doit tourner.",
            provider = provider_label,
        )
    } else if lower.contains("dns") || lower.contains("no address") || lower.contains("name") {
        format!(
            "🌐 Impossible de résoudre le nom d'hôte de {provider}. Check ta connexion internet ou DNS.",
            provider = provider_label,
        )
    } else if lower.contains("timeout") || lower.contains("timed out") {
        format!(
            "⏱️  Timeout en contactant {provider}. Check ta connexion ou VPN.\n\
             → Si le problème persiste, change de provider : sparrow route set nvidia",
            provider = provider_label,
        )
    } else if lower.contains("ssl") || lower.contains("tls") || lower.contains("certificate") {
        format!(
            "🔒 Erreur SSL/TLS avec {provider}. Vérifie tes certificats ou désactive le VPN.\n\
             → Détail : {error_msg}",
            provider = provider_label,
            error_msg = error_msg,
        )
    } else {
        format!(
            "📡 Problème de connexion vers {provider} : {error_msg}\n\
             → Check ta connexion ou VPN.\n\
             → Lance sparrow doctor pour un diagnostic complet.",
            provider = provider_label,
            error_msg = error_msg,
        )
    }
}

/// Config-related errors in French.
pub fn humanize_config_error(error_type: &str, context: &str) -> String {
    match error_type {
        "no_provider" => format!(
            "⚙️  Aucun provider configuré.\n\
             → Lance le setup : sparrow setup\n\
             → Ou définis une variable d'environnement : export {ctx}_API_KEY=\"ta-clé\"\n\
             → Providers gratuits dispos : NVIDIA (sparrow auth add nvidia), Groq, Gemini",
            ctx = context.to_uppercase(),
        ),
        "no_model" => format!(
            "⚙️  Aucun modèle défini pour le provider \"{ctx}\".\n\
             → Liste les modèles : sparrow model --list\n\
             → Définis un modèle : sparrow model --set {ctx}:<nom-du-modèle>",
            ctx = context,
        ),
        "no_config_file" => format!(
            "📄 Pas de fichier de config Sparrow trouvé.\n\
             → Premier lancement ? Lance sparrow setup\n\
             → Sinon, crée ~/.config/sparrow/config.toml manuellement",
        ),
        "config_parse_error" => format!(
            "📄 Erreur de parsing dans le fichier de config.\n\
             → Édite le fichier : sparrow config --edit\n\
             → Détail de l'erreur : {ctx}\n\
             → Format attendu : TOML valide (voir https://toml.io)",
            ctx = context,
        ),
        "invalid_autonomy" => format!(
            "⚙️  Niveau d'autonomie invalide : \"{ctx}\".\n\
             → Valeurs acceptées : supervised, trusted, autonomous\n\
             → Modifie : sparrow config --edit",
            ctx = context,
        ),
        "invalid_sandbox" => format!(
            "⚙️  Type de sandbox invalide : \"{ctx}\".\n\
             → Valeurs acceptées : local, local-hardened, docker\n\
             → Modifie : sparrow config --edit",
            ctx = context,
        ),
        "budget_exceeded" => format!(
            "💰 Budget dépassé !\n\
             → Budget actuel : ${ctx}\n\
             → Augmente le budget : sparrow config --edit (section [budget])\n\
             → Ou utilise un provider gratuit : sparrow route set nvidia",
            ctx = context,
        ),
        _ => format!(
            "⚠️  Erreur de configuration : {ctx}\n\
             → Lance sparrow doctor pour un diagnostic complet.\n\
             → Édite la config : sparrow config --edit",
            ctx = context,
        ),
    }
}

/// Best-effort translation of any `anyhow::Error` into French.
///
/// Walks the error chain looking for known patterns (HTTP status codes,
/// connection errors, etc.). Falls back to the raw error message when no
/// pattern matches.
pub fn humanize_anyhow(err: &anyhow::Error, provider_label: &str, model: &str) -> String {
    let msg = format!("{err:#}");

    // Try to extract an HTTP status code from the error chain
    if let Some(status) = extract_http_status(&msg) {
        return humanize_http_error(status, &msg, provider_label, model);
    }

    // Connection errors
    if msg.contains("Connection refused")
        || msg.contains("connect")
        || msg.contains("DNS")
        || msg.contains("timeout")
        || msg.contains("SSL")
        || msg.contains("TLS")
    {
        return humanize_connection_error(provider_label, &msg);
    }

    // Config errors
    if msg.contains("config") || msg.contains("toml") || msg.contains("parse") {
        return humanize_config_error("config_parse_error", &msg);
    }

    // Generic fallback with French flavour
    format!(
        "💥 Oups ! Une erreur est survenue : {msg}\n\
         → Lance sparrow doctor pour un diagnostic.\n\
         → Si le problème persiste, ouvre une issue : https://github.com/ucav/Sparrow/issues",
        msg = msg,
    )
}

/// Try to extract an HTTP status code from an error message string.
fn extract_http_status(msg: &str) -> Option<u16> {
    // Look for patterns like "HTTP 401", "status: 429", "500 Internal Server Error"
    let re = regex::Regex::new(r"(?i)(?:HTTP\s+|status:\s*|^\s*)(\d{3})\b").ok()?;
    re.captures(msg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u16>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unauthorized_anthropic() {
        let msg = humanize_http_error(401, "", "Anthropic", "claude-sonnet-4-6");
        assert!(msg.contains("invalide"));
        assert!(msg.contains("console.anthropic.com"));
        assert!(msg.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_rate_limit() {
        let msg = humanize_rate_limit("Groq", Some(30));
        assert!(msg.contains("trop de requêtes"));
        assert!(msg.contains("30 secondes"));
    }

    #[test]
    fn test_model_not_found() {
        let msg = humanize_model_not_found("NVIDIA NIM", "gpt-5-ultra");
        assert!(msg.contains("n'existe pas"));
        assert!(msg.contains("gpt-5-ultra"));
    }

    #[test]
    fn test_connection_refused() {
        let msg = humanize_connection_error("Ollama", "Connection refused (os error 111)");
        assert!(msg.contains("Impossible de contacter"));
        assert!(msg.contains("ollama serve"));
    }

    #[test]
    fn test_config_no_provider() {
        let msg = humanize_config_error("no_provider", "ANTHROPIC");
        assert!(msg.contains("Aucun provider"));
    }

    #[test]
    fn test_extract_http_status() {
        assert_eq!(extract_http_status("HTTP 401 Unauthorized"), Some(401));
        assert_eq!(extract_http_status("status: 429 Too Many Requests"), Some(429));
        assert_eq!(extract_http_status("500 Internal Server Error"), Some(500));
        assert_eq!(extract_http_status("no status here"), None);
    }
}
