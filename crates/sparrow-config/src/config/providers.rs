use serde::{Deserialize, Serialize};

use sparrow_providers::{LatencyClass, ModelCaps};

// ─── Auth flow types ─────────────────────────────────────────────────────────

/// Describes how a provider authenticates the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthFlow {
    /// Standard API key — stored as PROVIDER_API_KEY or via `sparrow auth add`.
    ApiKey,
    /// RFC 8628 Device Authorization Grant — browser-less, best for CLIs.
    /// `device_endpoint`: POST URL to start the flow.
    /// `token_endpoint`:  POST URL to poll for the token.
    /// `scope`:           Space-separated OAuth scopes to request.
    /// `client_id_env`:   Env var name for the client id (optional override).
    DeviceOAuth {
        device_endpoint: String,
        token_endpoint: String,
        scope: String,
        client_id_env: String,
    },
}

impl Default for AuthFlow {
    fn default() -> Self {
        AuthFlow::ApiKey
    }
}

/// Provider + model registry aligned with Hermes Agent (NousResearch/hermes-agent).
/// Source: https://github.com/NousResearch/hermes-agent/tree/main/plugins/model-providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDef {
    pub id: String,
    pub label: String,
    pub adapter: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub models: Vec<ModelDef>,
    pub tags: Vec<String>,
    pub notes: String,
    /// Authentication method. Defaults to `ApiKey` for backwards-compat.
    #[serde(default)]
    pub auth_flow: AuthFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDef {
    pub name: String,
    pub label: String,
    pub tags: Vec<String>,
    pub cost_input_per_mtok: f64,
    pub cost_output_per_mtok: f64,
    pub context_window: u64,
    pub recommended: bool,
}

pub fn provider_registry() -> Vec<ProviderDef> {
    vec![
        // ─── Ollama Cloud (Hermes: ollama-cloud) ──────────────────────
        ProviderDef {
            id: "ollama-cloud".into(),
            label: "Ollama Cloud".into(),
            adapter: "openai-compatible".into(),
            base_url: "http://localhost:11434/v1".into(),
            api_key_env: None,
            models: vec![
                ModelDef {
                    name: "qwen3.5:32b".into(),
                    label: "Qwen 3.5 32B".into(),
                    tags: vec!["free".into(), "code".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 32768,
                    recommended: true,
                },
                ModelDef {
                    name: "llama4:latest".into(),
                    label: "Llama 4".into(),
                    tags: vec!["free".into(), "strong".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: false,
                },
            ],
            tags: vec!["local".into(), "free".into()],
            notes: "Ollama Cloud. For self-hosted Ollama use 'custom' provider.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Anthropic ─────────────────────────────────────────────────
        ProviderDef {
            id: "anthropic".into(),
            label: "Anthropic".into(),
            adapter: "anthropic-messages".into(),
            base_url: "https://api.anthropic.com".into(),
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "claude-sonnet-4-6".into(),
                    label: "Claude Sonnet 4".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 3.0,
                    cost_output_per_mtok: 15.0,
                    context_window: 200000,
                    recommended: true,
                },
                ModelDef {
                    name: "claude-opus-4-8".into(),
                    label: "Claude Opus 4".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 15.0,
                    cost_output_per_mtok: 75.0,
                    context_window: 200000,
                    recommended: false,
                },
                ModelDef {
                    name: "claude-haiku-4-5".into(),
                    label: "Claude Haiku 4".into(),
                    tags: vec!["fast".into(), "cheap".into(), "tool_support".into()],
                    cost_input_per_mtok: 1.0,
                    cost_output_per_mtok: 5.0,
                    context_window: 200000,
                    recommended: false,
                },
            ],
            tags: vec!["strong".into(), "code".into(), "vision".into()],
            notes: "Best-in-class for complex code and reasoning.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── OpenAI Codex (Hermes: openai-codex) ───────────────────────
        ProviderDef {
            id: "openai-codex".into(),
            label: "OpenAI Codex".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: Some("OPENAI_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "gpt-5".into(),
                    label: "GPT-5".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 2.5,
                    cost_output_per_mtok: 10.0,
                    context_window: 128000,
                    recommended: true,
                },
                ModelDef {
                    name: "gpt-5-mini".into(),
                    label: "GPT-5 Mini".into(),
                    tags: vec![
                        "cheap".into(),
                        "fast".into(),
                        "code".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 0.15,
                    cost_output_per_mtok: 0.60,
                    context_window: 128000,
                    recommended: false,
                },
            ],
            tags: vec!["strong".into(), "code".into(), "vision".into()],
            notes: "OpenAI Codex — GPT models via OpenAI API.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── NVIDIA ────────────────────────────────────────────────────
        ProviderDef {
            id: "nvidia".into(),
            label: "NVIDIA NIM".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://integrate.api.nvidia.com/v1".into(),
            api_key_env: Some("NVIDIA_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "meta/llama-3.1-8b-instruct".into(),
                    label: "Llama 3.1 8B Instruct".into(),
                    tags: vec![
                        "fast".into(),
                        "free".into(),
                        "tool_support".into(),
                        "chat".into(),
                    ],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: true,
                },
                ModelDef {
                    name: "deepseek-ai/deepseek-v4-flash".into(),
                    label: "DeepSeek V4 Flash".into(),
                    tags: vec!["code".into(), "reasoning".into(), "free".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: false,
                },
                ModelDef {
                    name: "stepfun-ai/step-3.5-flash".into(),
                    label: "Step 3.5 Flash".into(),
                    tags: vec!["fast".into(), "free".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: true,
                },
                ModelDef {
                    name: "nvidia/llama-3.1-nemotron-nano-8b-v1".into(),
                    label: "Nemotron Nano 8B".into(),
                    tags: vec!["fast".into(), "code".into(), "free".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: false,
                },
                ModelDef {
                    name: "openai/gpt-oss-20b".into(),
                    label: "GPT-OSS 20B".into(),
                    tags: vec!["fast".into(), "free".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: false,
                },
                ModelDef {
                    name: "nvidia/nemotron-3-super-120b-a12b".into(),
                    label: "Nemotron Super 120B".into(),
                    tags: vec![
                        "strong".into(),
                        "reasoning".into(),
                        "code".into(),
                        "free".into(),
                    ],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: true,
                },
            ],
            tags: vec!["free".into(), "fast".into(), "strong".into(), "code".into()],
            notes: "NVIDIA API Catalog / NIM — free tier with API key; discovery expands this list from /v1/models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── OpenRouter ────────────────────────────────────────────────
        ProviderDef {
            id: "openrouter".into(),
            label: "OpenRouter".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key_env: Some("OPENROUTER_API_KEY".into()),
            models: vec![ModelDef {
                name: "openrouter/auto".into(),
                label: "Auto (best for task)".into(),
                tags: vec![
                    "strong".into(),
                    "code".into(),
                    "vision".into(),
                    "tool_support".into(),
                ],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 200000,
                recommended: true,
            }],
            tags: vec!["strong".into(), "multi".into()],
            notes: "200+ models via one API — auto-routes to best model.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── DeepSeek ──────────────────────────────────────────────────
        ProviderDef {
            id: "deepseek".into(),
            label: "DeepSeek".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "deepseek-chat".into(),
                    label: "DeepSeek V3".into(),
                    tags: vec!["cheap".into(), "code".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.27,
                    cost_output_per_mtok: 1.1,
                    context_window: 65536,
                    recommended: true,
                },
                ModelDef {
                    name: "deepseek-reasoner".into(),
                    label: "DeepSeek R1".into(),
                    tags: vec!["reasoning".into(), "strong".into()],
                    cost_input_per_mtok: 0.55,
                    cost_output_per_mtok: 2.19,
                    context_window: 65536,
                    recommended: false,
                },
            ],
            tags: vec!["cheap".into(), "code".into(), "reasoning".into()],
            notes: "DeepSeek — very competitive pricing, strong coding.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Gemini ────────────────────────────────────────────────────
        ProviderDef {
            id: "gemini".into(),
            label: "Google Gemini".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            api_key_env: Some("GEMINI_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "gemini-2.5-pro".into(),
                    label: "Gemini 2.5 Pro".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 1048576,
                    recommended: true,
                },
                ModelDef {
                    name: "gemini-2.5-flash".into(),
                    label: "Gemini 2.5 Flash".into(),
                    tags: vec![
                        "fast".into(),
                        "cheap".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 1048576,
                    recommended: false,
                },
            ],
            tags: vec!["strong".into(), "vision".into(), "free".into()],
            notes: "Google Gemini — 1M context window, free tier.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── xAI ───────────────────────────────────────────────────────
        ProviderDef {
            id: "xai".into(),
            label: "xAI (Grok)".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.x.ai/v1".into(),
            api_key_env: Some("XAI_API_KEY".into()),
            models: vec![ModelDef {
                name: "grok-3".into(),
                label: "Grok 3".into(),
                tags: vec!["strong".into(), "code".into(), "vision".into()],
                cost_input_per_mtok: 3.0,
                cost_output_per_mtok: 15.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["strong".into(), "code".into()],
            notes: "xAI Grok — strong reasoning and coding.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── HuggingFace ───────────────────────────────────────────────
        ProviderDef {
            id: "huggingface".into(),
            label: "Hugging Face".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api-inference.huggingface.co/v1".into(),
            api_key_env: Some("HF_TOKEN".into()),
            models: vec![ModelDef {
                name: "Qwen/Qwen3-235B-A22B".into(),
                label: "Qwen 3 235B".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["free".into()],
            notes: "Hugging Face Serverless Inference — free tier, many models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Nous Portal ───────────────────────────────────────────────
        ProviderDef {
            id: "nous".into(),
            label: "Nous Portal".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://portal.nousresearch.com/api/v1".into(),
            api_key_env: Some("NOUS_API_KEY".into()),
            models: vec![ModelDef {
                name: "hermes-3-70b".into(),
                label: "Hermes 3 70B".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["strong".into(), "code".into()],
            notes: "Nous Portal — one sub for models + web search + image gen + TTS + browser."
                .into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── NovitaAI ──────────────────────────────────────────────────
        ProviderDef {
            id: "novita".into(),
            label: "NovitaAI".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.novita.ai/v3/openai".into(),
            api_key_env: Some("NOVITA_API_KEY".into()),
            models: vec![ModelDef {
                name: "deepseek/deepseek-r1".into(),
                label: "DeepSeek R1".into(),
                tags: vec!["reasoning".into(), "strong".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 65536,
                recommended: true,
            }],
            tags: vec!["cheap".into(), "reasoning".into()],
            notes: "NovitaAI — AI-native cloud for Model API, Agent Sandbox, and GPU Cloud.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Alibaba ───────────────────────────────────────────────────
        ProviderDef {
            id: "alibaba".into(),
            label: "Alibaba Cloud".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".into(),
            api_key_env: Some("DASHSCOPE_API_KEY".into()),
            models: vec![ModelDef {
                name: "qwen-plus".into(),
                label: "Qwen Plus".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["cheap".into(), "code".into()],
            notes: "Alibaba Cloud DashScope — Qwen models via OpenAI-compatible API.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Bedrock (AWS) ─────────────────────────────────────────────
        ProviderDef {
            id: "bedrock".into(),
            label: "AWS Bedrock".into(),
            adapter: "bedrock".into(),
            base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            api_key_env: Some("AWS_ACCESS_KEY_ID".into()),
            models: vec![ModelDef {
                name: "anthropic.claude-sonnet-4-6".into(),
                label: "Claude Sonnet 4".into(),
                tags: vec![
                    "strong".into(),
                    "code".into(),
                    "vision".into(),
                    "tool_support".into(),
                ],
                cost_input_per_mtok: 3.0,
                cost_output_per_mtok: 15.0,
                context_window: 200000,
                recommended: true,
            }],
            tags: vec!["strong".into(), "code".into()],
            notes: "AWS Bedrock — managed foundation models. Requires AWS credentials.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Kimi/Moonshot ─────────────────────────────────────────────
        ProviderDef {
            id: "kimi-coding".into(),
            label: "Kimi Coding".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.moonshot.cn/v1".into(),
            api_key_env: Some("MOONSHOT_API_KEY".into()),
            models: vec![ModelDef {
                name: "moonshot-v1-auto".into(),
                label: "Moonshot Auto".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "Kimi/Moonshot — coding-focused models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── MiniMax ───────────────────────────────────────────────────
        ProviderDef {
            id: "minimax".into(),
            label: "MiniMax".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.minimax.chat/v1".into(),
            api_key_env: Some("MINIMAX_API_KEY".into()),
            models: vec![ModelDef {
                name: "abab7-chat".into(),
                label: "ABAB 7".into(),
                tags: vec!["strong".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["strong".into()],
            notes: "MiniMax — ABAB series models via OpenAI-compatible API.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Xiaomi MiMo ───────────────────────────────────────────────
        ProviderDef {
            id: "xiaomi".into(),
            label: "Xiaomi MiMo".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://platform.xiaomimimo.com/v1".into(),
            api_key_env: Some("XIAOMI_API_KEY".into()),
            models: vec![ModelDef {
                name: "mimo-v2".into(),
                label: "MiMo V2".into(),
                tags: vec!["strong".into(), "code".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "Xiaomi MiMo — coding models via Xiaomi platform.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── z.ai/GLM ──────────────────────────────────────────────────
        ProviderDef {
            id: "zai".into(),
            label: "z.ai / GLM".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.z.ai/v1".into(),
            api_key_env: Some("ZAI_API_KEY".into()),
            models: vec![ModelDef {
                name: "glm-4-plus".into(),
                label: "GLM-4 Plus".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "z.ai / GLM — ChatGLM models via OpenAI-compatible API.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── GMI Cloud ─────────────────────────────────────────────────
        ProviderDef {
            id: "gmi".into(),
            label: "GMI Cloud".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.gmi.cloud/v1".into(),
            api_key_env: Some("GMI_API_KEY".into()),
            models: vec![ModelDef {
                name: "llama-4-maverick".into(),
                label: "Llama 4 Maverick".into(),
                tags: vec!["strong".into(), "code".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["cheap".into()],
            notes: "GMI Cloud — GPU cloud for open-source model inference.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Arcee ─────────────────────────────────────────────────────
        ProviderDef {
            id: "arcee".into(),
            label: "Arcee".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.arcee.ai/v1".into(),
            api_key_env: Some("ARCEE_API_KEY".into()),
            models: vec![ModelDef {
                name: "arcee-virtuoso-small".into(),
                label: "Virtuoso Small".into(),
                tags: vec!["code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "Arcee — specialized coding models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── StepFun ───────────────────────────────────────────────────
        ProviderDef {
            id: "stepfun".into(),
            label: "StepFun".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.stepfun.com/v1".into(),
            api_key_env: Some("STEPFUN_API_KEY".into()),
            models: vec![ModelDef {
                name: "step-2-16k".into(),
                label: "Step-2 16K".into(),
                tags: vec!["strong".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 16384,
                recommended: true,
            }],
            tags: vec!["cheap".into()],
            notes: "StepFun — Step series models via OpenAI-compatible API.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Custom ────────────────────────────────────────────────────
        ProviderDef {
            id: "custom".into(),
            label: "Custom Endpoint".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://your-endpoint/v1".into(),
            api_key_env: Some("CUSTOM_API_KEY".into()),
            models: vec![ModelDef {
                name: "custom-model".into(),
                label: "Custom Model".into(),
                tags: vec!["custom".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 128000,
                recommended: true,
            }],
            tags: vec!["custom".into()],
            notes: "Bring your own OpenAI-compatible endpoint. Set base_url to your server.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Azure Foundry ─────────────────────────────────────────────
        ProviderDef {
            id: "azure-foundry".into(),
            label: "Azure AI Foundry".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://<your-resource>.openai.azure.com/openai/v1".into(),
            api_key_env: Some("AZURE_OPENAI_API_KEY".into()),
            models: vec![ModelDef {
                name: "gpt-5".into(),
                label: "GPT-5 (Azure)".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 128000,
                recommended: true,
            }],
            tags: vec!["strong".into(), "code".into()],
            notes:
                "Azure AI Foundry — OpenAI models on Azure. Set base_url with your resource name."
                    .into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Qwen OAuth ────────────────────────────────────────────────
        ProviderDef {
            id: "qwen-oauth".into(),
            label: "Qwen (OAuth)".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key_env: None,
            models: vec![ModelDef {
                name: "qwen-plus".into(),
                label: "Qwen Plus".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "Qwen via OAuth login — no API key needed, login via browser.".into(),
            auth_flow: AuthFlow::DeviceOAuth {
                device_endpoint: "https://oauth.aliyun.com/device/code".into(),
                token_endpoint:  "https://oauth.aliyun.com/device/token".into(),
                scope: "openid profile".into(),
                client_id_env: "QWEN_OAUTH_CLIENT_ID".into(),
            },
        },
        // ─── OpenCode Go (paid subscription tier) ─────────────────────
        ProviderDef {
            id: "opencode-go".into(),
            label: "OpenCode Go".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://opencode.ai/zen/go/v1".into(),
            api_key_env: Some("OPENCODE_GO_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "claude-sonnet-4-6".into(),
                    label: "Claude Sonnet 4.6 (Go)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 3.0,
                    cost_output_per_mtok: 15.0,
                    context_window: 200000,
                    recommended: true,
                },
                ModelDef {
                    name: "claude-opus-4-8".into(),
                    label: "Claude Opus 4 (Go)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 15.0,
                    cost_output_per_mtok: 75.0,
                    context_window: 200000,
                    recommended: false,
                },
                ModelDef {
                    name: "gpt-5".into(),
                    label: "GPT-5 (Go)".into(),
                    tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                    cost_input_per_mtok: 5.0,
                    cost_output_per_mtok: 20.0,
                    context_window: 200000,
                    recommended: false,
                },
                ModelDef {
                    name: "gemini-2-5-pro".into(),
                    label: "Gemini 2.5 Pro (Go)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 2.5,
                    cost_output_per_mtok: 15.0,
                    context_window: 1000000,
                    recommended: false,
                },
            ],
            tags: vec!["code".into(), "strong".into(), "multi".into(), "paid".into()],
            notes: "OpenCode Go subscription tier — full model library via go/v1 endpoint. Discovery expands the list from /v1/models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── OpenCode Zen ──────────────────────────────────────────────
        ProviderDef {
            id: "opencode-zen".into(),
            label: "OpenCode Zen".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://opencode.ai/zen/v1".into(),
            api_key_env: Some("OPENCODE_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "claude-sonnet-4-6".into(),
                    label: "Claude Sonnet 4.6 (via Zen)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "vision".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 3.0,
                    cost_output_per_mtok: 15.0,
                    context_window: 200000,
                    recommended: true,
                },
                ModelDef {
                    name: "qwen3.6-plus".into(),
                    label: "Qwen 3.6 Plus (via Zen)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 1.0,
                    cost_output_per_mtok: 3.0,
                    context_window: 262144,
                    recommended: false,
                },
                ModelDef {
                    name: "gpt-5-codex".into(),
                    label: "GPT-5 Codex (via Zen)".into(),
                    tags: vec![
                        "strong".into(),
                        "code".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 2.5,
                    cost_output_per_mtok: 10.0,
                    context_window: 200000,
                    recommended: false,
                },
            ],
            tags: vec!["code".into(), "strong".into(), "multi".into()],
            notes: "OpenCode Zen — curated gateway (Claude, GPT, Qwen, DeepSeek). Most models need credits or an OpenCode Go subscription. Discovery expands the list from /v1/models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── KiloCode ──────────────────────────────────────────────────
        ProviderDef {
            id: "kilocode".into(),
            label: "KiloCode".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.kilocode.ai/v1".into(),
            api_key_env: Some("KILOCODE_API_KEY".into()),
            models: vec![ModelDef {
                name: "kilocode".into(),
                label: "KiloCode".into(),
                tags: vec!["code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "KiloCode — coding-specialized models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Copilot (GitHub) ──────────────────────────────────────────
        ProviderDef {
            id: "copilot".into(),
            label: "GitHub Copilot".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.githubcopilot.com".into(),
            api_key_env: Some("GITHUB_TOKEN".into()),
            models: vec![ModelDef {
                name: "copilot".into(),
                label: "Copilot".into(),
                tags: vec!["code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 32768,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "GitHub Copilot — available with Copilot subscription.".into(),
            auth_flow: AuthFlow::DeviceOAuth {
                device_endpoint: "https://github.com/login/device/code".into(),
                token_endpoint:  "https://github.com/login/oauth/access_token".into(),
                scope: "read:user".into(),
                client_id_env: "GITHUB_OAUTH_CLIENT_ID".into(),
            },
        },
        // ─── Alibaba Coding Plan ───────────────────────────────────────
        ProviderDef {
            id: "alibaba-coding-plan".into(),
            label: "Alibaba Coding Plan".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".into(),
            api_key_env: Some("DASHSCOPE_API_KEY".into()),
            models: vec![ModelDef {
                name: "qwen-coder-plus".into(),
                label: "Qwen Coder Plus".into(),
                tags: vec!["code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["code".into()],
            notes: "Alibaba Coding Plan — Qwen Coder models for software development.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ═══ MERGED: additional providers not in Hermes but valid ═════

        // ─── Ollama (local, self-hosted) ───────────────────────────────
        ProviderDef {
            id: "ollama".into(),
            label: "Ollama (local)".into(),
            adapter: "ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            api_key_env: Some("OLLAMA_HOST".into()),
            models: vec![
                ModelDef {
                    name: "qwen3.5:32b".into(),
                    label: "Qwen 3.5 32B".into(),
                    tags: vec![
                        "local".into(),
                        "free".into(),
                        "code".into(),
                        "tool_support".into(),
                    ],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 32768,
                    recommended: true,
                },
                ModelDef {
                    name: "codellama:latest".into(),
                    label: "CodeLlama".into(),
                    tags: vec!["local".into(), "free".into(), "code".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 16384,
                    recommended: false,
                },
                ModelDef {
                    name: "mistral:latest".into(),
                    label: "Mistral (local)".into(),
                    tags: vec!["local".into(), "free".into(), "fast".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 32768,
                    recommended: false,
                },
            ],
            tags: vec!["local".into(), "free".into(), "offline".into()],
            notes: "Self-hosted models via Ollama. Install: https://ollama.com".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Groq ──────────────────────────────────────────────────────
        ProviderDef {
            id: "groq".into(),
            label: "Groq".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            api_key_env: Some("GROQ_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "llama-4-scout-17b-16e".into(),
                    label: "Llama 4 Scout".into(),
                    tags: vec!["fast".into(), "cheap".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: true,
                },
                ModelDef {
                    name: "deepseek-r1-distill-llama-70b".into(),
                    label: "DeepSeek R1 70B".into(),
                    tags: vec!["reasoning".into(), "strong".into()],
                    cost_input_per_mtok: 0.0,
                    cost_output_per_mtok: 0.0,
                    context_window: 131072,
                    recommended: false,
                },
            ],
            tags: vec!["fast".into(), "free".into()],
            notes: "Groq LPU — ultra-fast inference, free tier available.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Together AI ───────────────────────────────────────────────
        ProviderDef {
            id: "together".into(),
            label: "Together AI".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.together.xyz/v1".into(),
            api_key_env: Some("TOGETHER_API_KEY".into()),
            models: vec![ModelDef {
                name: "meta-llama/Llama-4-Maverick-17B-128E".into(),
                label: "Llama 4 Maverick".into(),
                tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.2,
                cost_output_per_mtok: 0.2,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["cheap".into(), "code".into()],
            notes: "Together AI — open-source models at low cost.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Cerebras ──────────────────────────────────────────────────
        ProviderDef {
            id: "cerebras".into(),
            label: "Cerebras".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.cerebras.ai/v1".into(),
            api_key_env: Some("CEREBRAS_API_KEY".into()),
            models: vec![ModelDef {
                name: "llama-4-scout-17b-16e".into(),
                label: "Llama 4 Scout".into(),
                tags: vec!["fast".into(), "cheap".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["fast".into(), "free".into()],
            notes: "Cerebras Wafer-Scale — fastest inference available.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Mistral ───────────────────────────────────────────────────
        ProviderDef {
            id: "mistral".into(),
            label: "Mistral AI".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.mistral.ai/v1".into(),
            api_key_env: Some("MISTRAL_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "mistral-large-latest".into(),
                    label: "Mistral Large".into(),
                    tags: vec!["strong".into(), "code".into(), "tool_support".into()],
                    cost_input_per_mtok: 2.0,
                    cost_output_per_mtok: 6.0,
                    context_window: 131072,
                    recommended: true,
                },
                ModelDef {
                    name: "mistral-small-latest".into(),
                    label: "Mistral Small".into(),
                    tags: vec!["cheap".into(), "fast".into()],
                    cost_input_per_mtok: 0.2,
                    cost_output_per_mtok: 0.6,
                    context_window: 32768,
                    recommended: false,
                },
            ],
            tags: vec!["strong".into(), "code".into()],
            notes: "Mistral AI — strong European models.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Fireworks AI ──────────────────────────────────────────────
        ProviderDef {
            id: "fireworks".into(),
            label: "Fireworks AI".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.fireworks.ai/inference/v1".into(),
            api_key_env: Some("FIREWORKS_API_KEY".into()),
            models: vec![ModelDef {
                name: "accounts/fireworks/models/llama-v4-maverick".into(),
                label: "Llama 4 Maverick".into(),
                tags: vec!["fast".into(), "code".into(), "tool_support".into()],
                cost_input_per_mtok: 0.2,
                cost_output_per_mtok: 0.2,
                context_window: 131072,
                recommended: true,
            }],
            tags: vec!["fast".into(), "code".into()],
            notes: "Fireworks AI — fast open-source model inference.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Perplexity ────────────────────────────────────────────────
        ProviderDef {
            id: "perplexity".into(),
            label: "Perplexity".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.perplexity.ai".into(),
            api_key_env: Some("PERPLEXITY_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "sonar-pro".into(),
                    label: "Sonar Pro".into(),
                    tags: vec!["search".into(), "web".into(), "tool_support".into()],
                    cost_input_per_mtok: 3.0,
                    cost_output_per_mtok: 15.0,
                    context_window: 200000,
                    recommended: true,
                },
                ModelDef {
                    name: "sonar".into(),
                    label: "Sonar".into(),
                    tags: vec!["search".into(), "fast".into(), "web".into()],
                    cost_input_per_mtok: 1.0,
                    cost_output_per_mtok: 1.0,
                    context_window: 127000,
                    recommended: false,
                },
            ],
            tags: vec!["search".into(), "web".into()],
            notes: "Perplexity Sonar — live web/search-focused model routing.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Cohere ────────────────────────────────────────────────────
        ProviderDef {
            id: "cohere".into(),
            label: "Cohere".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.cohere.com/compatibility/v1".into(),
            api_key_env: Some("COHERE_API_KEY".into()),
            models: vec![
                ModelDef {
                    name: "command-a-03-2025".into(),
                    label: "Command A".into(),
                    tags: vec!["strong".into(), "tool_support".into(), "enterprise".into()],
                    cost_input_per_mtok: 2.5,
                    cost_output_per_mtok: 10.0,
                    context_window: 256000,
                    recommended: true,
                },
                ModelDef {
                    name: "command-r7b-12-2024".into(),
                    label: "Command R7B".into(),
                    tags: vec!["fast".into(), "cheap".into(), "tool_support".into()],
                    cost_input_per_mtok: 0.15,
                    cost_output_per_mtok: 0.6,
                    context_window: 128000,
                    recommended: false,
                },
            ],
            tags: vec!["enterprise".into(), "tool_support".into()],
            notes: "Cohere Command models through the OpenAI-compatible endpoint.".into(),
            auth_flow: AuthFlow::default(),
        },
        // ─── Google (OAuth device flow) ────────────────────────────────
        ProviderDef {
            id: "google-oauth".into(),
            label: "Google (OAuth)".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            api_key_env: None,
            models: vec![ModelDef {
                name: "gemini-2.5-pro".into(),
                label: "Gemini 2.5 Pro".into(),
                tags: vec!["strong".into(), "vision".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 1_000_000,
                recommended: true,
            }],
            tags: vec!["strong".into()],
            notes: "Google Gemini via OAuth device flow — no API key required.".into(),
            auth_flow: AuthFlow::DeviceOAuth {
                device_endpoint: "https://oauth2.googleapis.com/device/code".into(),
                token_endpoint:  "https://oauth2.googleapis.com/token".into(),
                scope: "openid https://www.googleapis.com/auth/generative-language".into(),
                client_id_env: "GOOGLE_OAUTH_CLIENT_ID".into(),
            },
        },
        // ─── Microsoft (OAuth device flow) ────────────────────────────
        ProviderDef {
            id: "microsoft-oauth".into(),
            label: "Microsoft (OAuth)".into(),
            adapter: "openai-compatible".into(),
            base_url: "https://api.cognitive.microsoft.com/openai/v1".into(),
            api_key_env: None,
            models: vec![ModelDef {
                name: "gpt-4o".into(),
                label: "GPT-4o (Azure)".into(),
                tags: vec!["strong".into(), "vision".into(), "tool_support".into()],
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                context_window: 128000,
                recommended: true,
            }],
            tags: vec!["strong".into()],
            notes: "Microsoft Azure OpenAI via OAuth device flow.".into(),
            auth_flow: AuthFlow::DeviceOAuth {
                device_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode".into(),
                token_endpoint:  "https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
                scope: "openid profile".into(),
                client_id_env: "MICROSOFT_OAUTH_CLIENT_ID".into(),
            },
        },
    ]
}

pub fn find_provider(id: &str) -> Option<ProviderDef> {
    provider_registry().into_iter().find(|p| p.id == id)
}

/// Return all providers that use an OAuth device flow (not a plain API key).
pub fn list_oauth_providers() -> Vec<ProviderDef> {
    provider_registry()
        .into_iter()
        .filter(|p| p.auth_flow != AuthFlow::ApiKey)
        .collect()
}

pub fn find_model(provider_id: &str, model_name: &str) -> Option<ModelDef> {
    find_provider(provider_id).and_then(|p| p.models.into_iter().find(|m| m.name == model_name))
}

pub fn default_models(provider_id: &str) -> Vec<String> {
    find_provider(provider_id)
        .map(|p| {
            let recommended: Vec<String> = p
                .models
                .iter()
                .filter(|m| m.recommended)
                .map(|m| m.name.clone())
                .collect();
            if recommended.is_empty() {
                p.models.into_iter().map(|m| m.name).collect()
            } else {
                recommended
            }
        })
        .unwrap_or_default()
}

/// Infer capabilities for a model that is NOT in the static registry (e.g. a
/// model returned by live discovery). Heuristics on the model name let the
/// router distinguish a strong 120B coder from an 8B flash model instead of
/// treating every discovered model as identical default caps.
pub fn infer_caps_from_name(model_name: &str) -> ModelCaps {
    let n = model_name.to_ascii_lowercase();

    let vision = n.contains("vision")
        || n.contains("-vl")
        || n.contains("vl-")
        || n.contains("multimodal")
        || n.contains("omni");

    // Coding / tool-use signal
    let tools = n.contains("coder")
        || n.contains("code")
        || n.contains("instruct")
        || n.contains("chat")
        || n.contains("nemotron")
        || n.contains("qwen")
        || n.contains("llama")
        || n.contains("mistral")
        || n.contains("deepseek")
        || n.contains("gpt")
        || n.contains("glm");

    // Size / strength signal → latency + context window
    let is_large = [
        "70b", "72b", "120b", "122b", "175b", "180b", "235b", "253b", "340b", "397b", "405b",
        "480b", "675b", "ultra", "-large", "super",
    ]
    .iter()
    .any(|t| n.contains(t));
    let is_small = n.contains("flash")
        || n.contains("nano")
        || n.contains("mini")
        || n.contains("lite")
        || n.contains("-small")
        || n.contains("1b")
        || n.contains("2b")
        || n.contains("3b")
        || n.contains("7b")
        || n.contains("8b")
        || n.contains("9b");

    // Vendor-specific long-context families. These take precedence over the
    // generic size heuristic so a discovered "deepseek-v4-pro" (which the
    // vendor ships with 1M context) doesn't get clamped to the small default.
    // Order matters: most specific match first.
    let vendor_ctx: Option<u64> = if n.contains("gemini")
        && (n.contains("1.5") || n.contains("2.0") || n.contains("2.5") || n.contains("3."))
    {
        Some(2_000_000) // Gemini 1.5+ Pro family ships 1M–2M context
    } else if n.contains("deepseek-v4-pro")
        || n.contains("deepseek-v4.5")
        || n.contains("deepseek-v5")
        || (n.contains("deepseek") && n.contains("pro"))
    {
        Some(1_000_000) // DeepSeek V4 Pro and successors
    } else if n.contains("deepseek-v4")
        || n.contains("deepseek-r1")
        || n.contains("deepseek-reasoner")
    {
        Some(131_072) // DeepSeek V4 flash / R1: 128k
    } else if n.contains("deepseek-chat")
        || n.contains("deepseek-coder")
        || n.contains("deepseek-v3")
    {
        Some(65_536) // Older DeepSeek V3 chat/coder: 64k
    } else if n.contains("qwen3") && (n.contains("235b") || n.contains("max") || n.contains("plus"))
    {
        Some(1_000_000) // Qwen3 Max / Plus: 1M
    } else if n.contains("qwen") && (n.contains("turbo") || n.contains("plus") || n.contains("max"))
    {
        Some(1_000_000) // Qwen Turbo / Plus / Max
    } else if n.contains("qwen3") || n.contains("qwen2.5") {
        Some(131_072)
    } else if n.contains("gpt-4.1")
        || n.contains("gpt-5")
        || n.contains("o1")
        || n.contains("o3")
        || n.contains("o4")
    {
        Some(1_000_000) // GPT-4.1, GPT-5, o-series
    } else if n.contains("gpt-4o") || n.contains("gpt-4-turbo") {
        Some(128_000)
    } else if n.contains("claude-sonnet-4")
        || n.contains("claude-opus-4")
        || n.contains("claude-haiku-4")
    {
        Some(200_000)
    } else if n.contains("claude-3.5") || n.contains("claude-3-5") {
        Some(200_000)
    } else if n.contains("claude") {
        Some(200_000)
    } else if n.contains("llama-3.3") || n.contains("llama3.3") || n.contains("llama-4") {
        Some(128_000)
    } else if n.contains("mistral-large") || n.contains("mixtral") || n.contains("codestral") {
        Some(128_000)
    } else if n.contains("grok-3") || n.contains("grok-4") {
        Some(131_072)
    } else if n.contains("kimi-k2") || n.contains("moonshot-v1-128k") {
        Some(200_000)
    } else if n.contains("kimi") || n.contains("moonshot") {
        Some(128_000)
    } else if n.contains("minimax-m") || n.contains("minimax-text-01") {
        Some(1_000_000)
    } else if n.contains("step-3") || n.contains("stepfun") {
        Some(131_072)
    } else if n.contains("glm-4.6") || n.contains("glm-5") {
        Some(200_000)
    } else if n.contains("glm-4") || n.contains("glm-z1") {
        Some(131_072)
    } else if n.contains("longctx") || n.contains("long-ctx") || n.contains("1m") {
        Some(1_000_000)
    } else if n.contains("128k") {
        Some(128_000)
    } else if n.contains("200k") {
        Some(200_000)
    } else if n.contains("256k") {
        Some(262_144)
    } else if n.contains("512k") {
        Some(524_288)
    } else {
        None
    };

    let (latency, context_window) = if let Some(ctx) = vendor_ctx {
        let lat = if is_small {
            LatencyClass::Fast
        } else if is_large || ctx >= 200_000 {
            LatencyClass::Slow
        } else {
            LatencyClass::Medium
        };
        (lat, ctx)
    } else if is_large {
        (LatencyClass::Slow, 131_072)
    } else if is_small {
        (LatencyClass::Fast, 32_768)
    } else {
        (LatencyClass::Medium, 65_536)
    };

    // max_output scales with context but caps at 32k tokens to avoid
    // accidentally requesting more output than any current provider supports.
    let max_output = (context_window / 8).clamp(4_096, 32_000);

    let (cost_input_per_mtok, cost_output_per_mtok) =
        if n.contains("deepseek-r1") || n.contains("deepseek-reasoner") {
            (0.55, 2.19)
        } else if n.contains("deepseek") {
            (0.27, 1.10)
        } else {
            (0.0, 0.0)
        };

    ModelCaps {
        context_window,
        max_output,
        tools,
        vision,
        cost_input_per_mtok,
        cost_output_per_mtok,
        latency,
    }
}

pub fn model_caps(provider_id: &str, model_name: &str) -> ModelCaps {
    let Some(model) = find_model(provider_id, model_name) else {
        // Not in the static registry → infer from the model name (discovery path).
        return infer_caps_from_name(model_name);
    };

    let latency = if model.tags.iter().any(|t| t == "fast") {
        LatencyClass::Fast
    } else if model.tags.iter().any(|t| t == "strong" || t == "reasoning") {
        LatencyClass::Slow
    } else {
        LatencyClass::Medium
    };

    ModelCaps {
        context_window: model.context_window,
        max_output: model.context_window.min(32_000).max(4_096),
        tools: model
            .tags
            .iter()
            .any(|t| t == "tool_support" || t == "code"),
        vision: model.tags.iter().any(|t| t == "vision"),
        cost_input_per_mtok: model.cost_input_per_mtok,
        cost_output_per_mtok: model.cost_output_per_mtok,
        latency,
    }
}

pub fn onboarding_providers() -> Vec<ProviderDef> {
    provider_registry()
}
