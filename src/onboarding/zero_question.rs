use std::collections::HashMap;

use crate::config::{Budget, Config, ConfigStore, ProviderConfig, providers};

const LOCAL_PROVIDER_ID: &str = "ollama";

/// v0.9 first launch path: create a usable config without asking questions.
///
/// The older conversational setup still exists for pro users, but the default
/// experience must reach "ready" before the user has to decide anything.
pub async fn prepare_default_launch(
    config: &Config,
    store: &dyn ConfigStore,
) -> anyhow::Result<Config> {
    let mut next = config.clone();
    next.experience.mode = "auto".into();
    next.experience.language = "auto".into();
    next.budget = Budget {
        daily_usd: next.budget.daily_usd.min(5.0),
        session_usd: next.budget.session_usd.min(0.50),
        max_wall_secs: next.budget.max_wall_secs.or(Some(300)),
        max_tokens: next.budget.max_tokens,
    };

    add_env_providers(&mut next.providers);
    ensure_local_fallback(&mut next.providers);
    prefer_free_first(&mut next);

    store.save(&next)?;
    Ok(next)
}

pub fn ready_message() -> &'static str {
    "Sparrow est prêt. Qu’est-ce qu’on règle aujourd’hui ?"
}

fn add_env_providers(providers_map: &mut HashMap<String, ProviderConfig>) {
    for def in providers::provider_registry() {
        let Some(api_key_env) = def.api_key_env.clone() else {
            continue;
        };
        let Ok(value) = std::env::var(&api_key_env) else {
            continue;
        };
        if value.trim().is_empty() || providers_map.contains_key(&def.id) {
            continue;
        }
        providers_map.insert(
            def.id.clone(),
            ProviderConfig {
                adapter: def.adapter,
                base_url: Some(def.base_url),
                models: providers::default_models(&def.id),
                api_key_env: Some(api_key_env),
            },
        );
    }
}

fn ensure_local_fallback(providers_map: &mut HashMap<String, ProviderConfig>) {
    providers_map
        .entry(LOCAL_PROVIDER_ID.into())
        .or_insert_with(|| ProviderConfig {
            adapter: "ollama".into(),
            base_url: Some(
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into()),
            ),
            models: vec!["qwen3.5:32b".into(), "llama4:latest".into()],
            api_key_env: None,
        });
}

fn prefer_free_first(config: &mut Config) {
    config.routing.free_first = true;
    config.routing.on_budget = "downgrade".into();
    config.routing.routing_mode = "auto".into();
    config.routing.preferred_provider = None;
    config.routing.preferred_model = None;

    let first_remote = config
        .providers
        .keys()
        .find(|name| name.as_str() != LOCAL_PROVIDER_ID)
        .cloned()
        .unwrap_or_else(|| LOCAL_PROVIDER_ID.into());

    config.routing.policy = HashMap::from([
        ("trivial".into(), LOCAL_PROVIDER_ID.into()),
        ("small".into(), first_remote.clone()),
        ("medium".into(), first_remote.clone()),
        ("hard".into(), first_remote.clone()),
        ("vision".into(), first_remote),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemoryStore;

    impl ConfigStore for MemoryStore {
        fn load(&self) -> anyhow::Result<Config> {
            Ok(Config::default())
        }

        fn save(&self, c: &Config) -> anyhow::Result<()> {
            assert!(c.providers.contains_key(LOCAL_PROVIDER_ID));
            assert_eq!(c.routing.policy.get("trivial").unwrap(), LOCAL_PROVIDER_ID);
            assert!(c.budget.session_usd <= 0.50);
            Ok(())
        }
    }

    #[tokio::test]
    async fn first_launch_is_zero_question_and_free_first() {
        let prepared = prepare_default_launch(&Config::default(), &MemoryStore)
            .await
            .unwrap();

        assert_eq!(prepared.experience.mode, "auto");
        assert_eq!(prepared.routing.on_budget, "downgrade");
        assert!(prepared.providers.contains_key(LOCAL_PROVIDER_ID));
        assert_eq!(
            ready_message(),
            "Sparrow est prêt. Qu’est-ce qu’on règle aujourd’hui ?"
        );
    }
}
