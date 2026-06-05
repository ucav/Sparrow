//! Provider auto-detection: scan environment for API keys, test connectivity
//! with lightweight API calls, rank providers by cost tier (free > paid),
//! and return a list of ready-to-use providers.
//!
//! Integrates with the first-run wizard in [`crate::onboarding::wizard`].

use crate::config::providers::{find_provider, provider_registry, ProviderDef};
use serde::{Deserialize, Serialize};

// ─── Detection result types ──────────────────────────────────────────────────

/// The result of scanning for one provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProvider {
    /// Registry id (e.g. "anthropic", "nvidia")
    pub id: String,
    /// Human label (e.g. "Anthropic", "NVIDIA NIM")
    pub label: String,
    /// Whether we found an API key in the environment
    pub key_found: bool,
    /// The env var name if applicable
    pub env_var: Option<String>,
    /// Cost tier
    pub tier: ProviderTier,
    /// Whether we successfully validated the key with a lightweight API call
    pub validated: Option<bool>,
    /// Error message if validation failed
    pub validation_error: Option<String>,
    /// Signup URL for getting a key
    pub signup_url: Option<String>,
    /// Whether this provider is recommended for the user
    pub recommended: bool,
    /// Short description for the wizard UI
    pub description: String,
}

/// Cost tier for ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProviderTier {
    /// Completely free (NVIDIA NIM, Groq free tier, Gemini free tier)
    Free = 0,
    /// Has a generous free tier (some paid models but free tier exists)
    FreeTier = 1,
    /// Paid but cheap (DeepSeek, etc.)
    Cheap = 2,
    /// Paid, standard pricing
    Paid = 3,
    /// Requires signup / no key found
    RequiresSignup = 4,
}

// ─── Known API key environment variables ─────────────────────────────────────

/// List of all known API key env vars and their associated provider ids.
const KNOWN_API_KEY_ENVS: &[(&str, &str)] = &[
    ("OPENAI_API_KEY", "openai-codex"),
    ("ANTHROPIC_API_KEY", "anthropic"),
    ("GEMINI_API_KEY", "gemini"),
    ("GROQ_API_KEY", "groq"),
    ("NVIDIA_API_KEY", "nvidia"),
    ("DEEPSEEK_API_KEY", "deepseek"),
    ("OPENROUTER_API_KEY", "openrouter"),
    ("XAI_API_KEY", "xai"),
    ("HF_TOKEN", "huggingface"),
    ("NOUS_API_KEY", "nous"),
    ("NOVITA_API_KEY", "novita"),
    ("DASHSCOPE_API_KEY", "alibaba"),
    ("MOONSHOT_API_KEY", "kimi-coding"),
    ("MISTRAL_API_KEY", "mistral"),
    ("TOGETHER_API_KEY", "together"),
    ("CEREBRAS_API_KEY", "cerebras"),
    ("FIREWORKS_API_KEY", "fireworks"),
    ("PERPLEXITY_API_KEY", "perplexity"),
    ("COHERE_API_KEY", "cohere"),
    ("AWS_ACCESS_KEY_ID", "bedrock"),
    ("COPILOT_TOKEN", "copilot"),
];

// ─── Environment scanning ────────────────────────────────────────────────────

/// Scan the environment for all known API keys.
///
/// Returns a map of provider_id → (env_var_name, key_value).
pub fn scan_environment() -> Vec<(&'static str, &'static str, String)> {
    KNOWN_API_KEY_ENVS
        .iter()
        .filter_map(|&(env_var, provider_id)| {
            std::env::var(env_var)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(|key| (provider_id, env_var, key))
        })
        .collect()
}

/// Detect all providers with their status.
///
/// Returns a vector of [`DetectedProvider`] sorted by tier (free first), then
/// by whether a key was found.
pub fn detect_all_providers() -> Vec<DetectedProvider> {
    let env_keys = scan_environment();
    let provider_map: std::collections::HashMap<&str, &(&str, &str, String)> = env_keys
        .iter()
        .map(|(pid, env, _)| (*pid, (pid, env, &String::new())))
        .collect::<std::collections::HashMap<_, _>>()
        // We actually need to keep the key values, so rebuild:
        ;
    // Rebuild properly
    let env_keys_map: std::collections::HashMap<&str, (&str, String)> = env_keys
        .iter()
        .map(|(pid, env, key)| (*pid, (*env, key.clone())))
        .collect();

    let mut providers: Vec<DetectedProvider> = Vec::new();

    for def in provider_registry() {
        let key_info = env_keys_map.get(def.id.as_str());

        let tier = classify_tier(&def);
        let signup_url = signup_url_for(&def);

        let description = match tier {
            ProviderTier::Free => format!(
                "Gratuit — {}. Modèle recommandé : {}",
                def.notes.trim_end_matches('.'),
                def.models
                    .iter()
                    .find(|m| m.recommended)
                    .map(|m| m.name.as_str())
                    .unwrap_or(def.models.first().map(|m| m.name.as_str()).unwrap_or("N/A")),
            ),
            _ => def.notes.clone(),
        };

        providers.push(DetectedProvider {
            id: def.id.clone(),
            label: def.label.clone(),
            key_found: key_info.is_some(),
            env_var: key_info.map(|(env, _)| env.to_string()),
            tier,
            validated: None,
            validation_error: None,
            signup_url,
            recommended: def.models.iter().any(|m| m.recommended),
            description,
        });
    }

    // Sort: free first, then by key found
    providers.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| b.key_found.cmp(&a.key_found)) // key found first within same tier
            .then_with(|| a.label.cmp(&b.label))
    });

    providers
}

/// Classify a provider definition into a cost tier.
fn classify_tier(def: &ProviderDef) -> ProviderTier {
    // Check tags first
    if def.tags.iter().any(|t| t == "free") {
        return ProviderTier::Free;
    }

    // Check if any model has zero cost (free tier)
    let all_free = !def.models.is_empty()
        && def
            .models
            .iter()
            .all(|m| m.cost_input_per_mtok == 0.0 && m.cost_output_per_mtok == 0.0);

    if all_free {
        return ProviderTier::Free;
    }

    let has_free_models = def
        .models
        .iter()
        .any(|m| m.cost_input_per_mtok == 0.0 && m.cost_output_per_mtok == 0.0);

    if has_free_models {
        return ProviderTier::FreeTier;
    }

    // Cheap if cheapest model < $1/M input tokens
    let cheapest_input = def
        .models
        .iter()
        .map(|m| m.cost_input_per_mtok)
        .fold(f64::MAX, f64::min);

    if cheapest_input < 1.0 {
        return ProviderTier::Cheap;
    }

    ProviderTier::Paid
}

/// Get the signup URL for a provider.
fn signup_url_for(def: &ProviderDef) -> Option<String> {
    match def.id.as_str() {
        "anthropic" => Some("https://console.anthropic.com/settings/keys".into()),
        "openai-codex" => Some("https://platform.openai.com/api-keys".into()),
        "gemini" => Some("https://aistudio.google.com/app/apikey".into()),
        "groq" => Some("https://console.groq.com/keys".into()),
        "nvidia" => Some("https://build.nvidia.com/explore/discover".into()),
        "deepseek" => Some("https://platform.deepseek.com/api_keys".into()),
        "openrouter" => Some("https://openrouter.ai/keys".into()),
        "xai" => Some("https://console.x.ai/".into()),
        "huggingface" => Some("https://huggingface.co/settings/tokens".into()),
        "nous" => Some("https://portal.nousresearch.com".into()),
        "novita" => Some("https://novita.ai/dashboard/key".into()),
        "alibaba" => Some("https://bailian.console.aliyun.com".into()),
        "kimi-coding" => Some("https://platform.moonshot.cn/console".into()),
        "mistral" => Some("https://console.mistral.ai/api-keys/".into()),
        "together" => Some("https://api.together.xyz/settings/api-keys".into()),
        "cerebras" => Some("https://cloud.cerebras.ai/".into()),
        "fireworks" => Some("https://fireworks.ai/api-keys".into()),
        "perplexity" => Some("https://www.perplexity.ai/settings/api".into()),
        "cohere" => Some("https://dashboard.cohere.com/api-keys".into()),
        _ => None,
    }
}

/// Check if the `gh` CLI is installed (for GitHub Copilot integration).
pub fn gh_cli_installed() -> bool {
    std::process::Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Test a provider's API key with a minimal request.
///
/// Returns `Ok(())` if the key is valid, `Err(msg)` with a French error
/// message otherwise.
pub async fn validate_api_key(provider_id: &str, api_key: &str) -> Result<(), String> {
    let def = match find_provider(provider_id) {
        Some(d) => d,
        None => {
            return Err(format!(
                "Provider \"{provider_id}\" inconnu dans le registre Sparrow."
            ));
        }
    };

    // Build a minimal request based on the adapter type
    match def.adapter.as_str() {
        "anthropic-messages" => validate_anthropic_key(api_key).await,
        "openai-compatible" => validate_openai_compatible_key(&def.base_url, api_key).await,
        "ollama" => {
            // Ollama doesn't need an API key — just check if it's reachable
            validate_ollama_connection(&def.base_url).await
        }
        _ => validate_openai_compatible_key(&def.base_url, api_key).await,
    }
}

/// Validate an Anthropic API key with a GET to /v1/models.
async fn validate_anthropic_key(api_key: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Erreur client HTTP: {e}"))?;

    let resp = client
        .get("https://api.anthropic.com/v1/models?limit=1")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Timeout — le serveur Anthropic ne répond pas. Check ta connexion.".into()
            } else if e.is_connect() {
                "Impossible de contacter api.anthropic.com. Vérifie ta connexion ou VPN.".into()
            } else {
                format!("Erreur réseau : {e}")
            }
        })?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 | 403 => Err("Clé API Anthropic invalide. Vérifie ta clé sur https://console.anthropic.com/settings/keys".into()),
        429 => Err("Rate limit Anthropic — trop de requêtes. Réessaie dans quelques secondes.".into()),
        s => Err(format!("Erreur HTTP {s} du serveur Anthropic.")),
    }
}

/// Validate an OpenAI-compatible API key with a GET to /v1/models?limit=1.
async fn validate_openai_compatible_key(base_url: &str, api_key: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Erreur client HTTP: {e}"))?;

    let url = format!("{}/models?limit=1", base_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Timeout — le serveur à {url} ne répond pas. Check ta connexion.")
            } else if e.is_connect() {
                format!("Impossible de contacter {url}. Vérifie ta connexion ou VPN.")
            } else {
                format!("Erreur réseau : {e}")
            }
        })?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 | 403 => Err("Clé API invalide. Vérifie ta clé.".into()),
        404 => {
            // Some providers don't have /v1/models — try a chat completions endpoint instead
            validate_with_chat_request(base_url, api_key).await
        }
        429 => Err("Rate limit — trop de requêtes. Réessaie dans quelques secondes.".into()),
        s => Err(format!("Erreur HTTP {s}.")),
    }
}

/// Fallback validation: send a minimal chat completion request (1 token max).
async fn validate_with_chat_request(base_url: &str, api_key: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Erreur client HTTP: {e}"))?;

    let url = format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "model": "gpt-3.5-turbo",  // widely supported model name for testing
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
        "temperature": 0.0,
    });

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Timeout — le serveur ne répond pas.".into()
            } else if e.is_connect() {
                format!("Impossible de contacter {url}.")
            } else {
                format!("Erreur réseau : {e}")
            }
        })?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 | 403 => Err("Clé API invalide.".into()),
        404 => Err("Endpoint chat/completions introuvable. L'URL de base est peut-être incorrecte.".into()),
        429 => Err("Rate limit — trop de requêtes.".into()),
        s => {
            // Even 400/422 is "good" — it means the key was accepted, just the model
            // name was wrong.
            if s == 400 || s == 422 {
                Ok(())
            } else {
                Err(format!("Erreur HTTP {s}."))
            }
        }
    }
}

/// Validate an Ollama connection (no API key needed).
async fn validate_ollama_connection(base_url: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Erreur client HTTP: {e}"))?;

    let root = base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1");
    let url = format!("{root}/api/tags");

    let resp = client.get(&url).send().await.map_err(|e| {
        if e.is_connect() {
            format!(
                "Ollama ne tourne pas sur {root}.\n\
                 → Lance `ollama serve` dans un autre terminal.\n\
                 → Ou installe Ollama : https://ollama.com"
            )
        } else {
            format!("Erreur réseau : {e}")
        }
    })?;

    match resp.status().as_u16() {
        200 => Ok(()),
        s => Err(format!("Ollama a répondu HTTP {s}. Vérifie que le serveur tourne.")),
    }
}

/// Run validation for all detected providers with keys.
///
/// Returns the list of providers with `validated` fields populated.
pub async fn validate_detected_providers(
    providers: &mut [DetectedProvider],
) {
    for p in providers.iter_mut() {
        if !p.key_found {
            p.validated = Some(false);
            p.validation_error = Some("Aucune clé API trouvée dans l'environnement.".into());
            continue;
        }

        let env_var = match &p.env_var {
            Some(env) => env.clone(),
            None => {
                p.validated = Some(false);
                p.validation_error = Some("Variable d'environnement inconnue.".into());
                continue;
            }
        };

        let api_key = match std::env::var(&env_var) {
            Ok(k) if !k.trim().is_empty() => k,
            _ => {
                p.validated = Some(false);
                p.validation_error = Some(format!("Variable {env_var} vide."));
                continue;
            }
        };

        match validate_api_key(&p.id, &api_key).await {
            Ok(()) => {
                p.validated = Some(true);
                p.validation_error = None;
            }
            Err(e) => {
                p.validated = Some(false);
                p.validation_error = Some(e);
            }
        }
    }
}

/// Get a summary list of ready-to-use providers (key found + validated).
pub fn ready_providers(providers: &[DetectedProvider]) -> Vec<&DetectedProvider> {
    providers
        .iter()
        .filter(|p| p.key_found && p.validated == Some(true))
        .collect()
}

/// Get a list of free providers (regardless of key status), for suggestions.
pub fn free_providers(providers: &[DetectedProvider]) -> Vec<&DetectedProvider> {
    providers
        .iter()
        .filter(|p| matches!(p.tier, ProviderTier::Free))
        .collect()
}
