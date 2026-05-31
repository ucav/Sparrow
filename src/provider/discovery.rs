use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

#[async_trait]
pub trait ModelDiscovery: Send + Sync {
    async fn fetch_model_names(&self, base_url: &str, api_key: &str)
    -> anyhow::Result<Vec<String>>;
}

pub struct OpenAICompatDiscovery;
pub struct AnthropicDiscovery;
pub struct OllamaDiscovery;

#[async_trait]
impl ModelDiscovery for OpenAICompatDiscovery {
    async fn fetch_model_names(
        &self,
        base_url: &str,
        api_key: &str,
    ) -> anyhow::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let mut request = client.get(url);
        if !api_key.trim().is_empty() {
            request = request.bearer_auth(api_key);
        }
        let value: Value = request.send().await?.error_for_status()?.json().await?;
        let models = value
            .get("data")
            .and_then(|data| data.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("id").and_then(|id| id.as_str()))
                    .filter(|id| is_chat_model_id(id))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}

#[async_trait]
impl ModelDiscovery for AnthropicDiscovery {
    async fn fetch_model_names(
        &self,
        _base_url: &str,
        api_key: &str,
    ) -> anyhow::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let value: Value = client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(value
            .get("data")
            .and_then(|data| data.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("id").and_then(|id| id.as_str()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }
}

#[async_trait]
impl ModelDiscovery for OllamaDiscovery {
    async fn fetch_model_names(
        &self,
        base_url: &str,
        _api_key: &str,
    ) -> anyhow::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let root = base_url.trim_end_matches('/').trim_end_matches("/v1");
        let value: Value = client
            .get(format!("{}/api/tags", root))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(value
            .get("models")
            .and_then(|models| models.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("name").and_then(|name| name.as_str()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }
}

pub async fn discover_models(
    adapter: &str,
    base_url: &str,
    api_key: &str,
) -> anyhow::Result<Vec<String>> {
    match adapter {
        "anthropic-messages" => {
            AnthropicDiscovery
                .fetch_model_names(base_url, api_key)
                .await
        }
        "ollama" => OllamaDiscovery.fetch_model_names(base_url, api_key).await,
        _ => {
            OpenAICompatDiscovery
                .fetch_model_names(base_url, api_key)
                .await
        }
    }
}

fn is_chat_model_id(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    !["embedding", "tts", "dall-e", "whisper", "moderation"]
        .iter()
        .any(|needle| id.contains(needle))
}
