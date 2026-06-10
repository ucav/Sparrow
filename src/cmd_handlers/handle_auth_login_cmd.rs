// src/cmd_handlers/handle_auth_login_cmd.rs
use super::prelude::*;
use super::prelude::*;
pub async fn handle_auth_login(
    provider: &str,
    client_id: Option<String>,
    auth: &sparrow::auth::store::ChainedAuthStore,
) -> anyhow::Result<()> {
    use sparrow::auth::AuthStore;
    use sparrow::config::providers::{AuthFlow, find_provider, list_oauth_providers};
    use sparrow::extras::OAuthFlow;

    let def = find_provider(provider).ok_or_else(|| {
        let oauth_ids: Vec<String> = list_oauth_providers().into_iter().map(|p| p.id).collect();
        anyhow::anyhow!(
            "Unknown provider '{}'. OAuth-capable providers: {}.\nFor API-key providers use 'sparrow auth add {}'.",
            provider,
            oauth_ids.join(", "),
            provider,
        )
    })?;

    let (device_ep, token_ep, scope, cid_env) = match &def.auth_flow {
        AuthFlow::DeviceOAuth {
            device_endpoint,
            token_endpoint,
            scope,
            client_id_env,
        } => (
            device_endpoint.clone(),
            token_endpoint.clone(),
            scope.clone(),
            client_id_env.clone(),
        ),
        AuthFlow::ApiKey => {
            anyhow::bail!(
                "Provider '{}' uses API-key auth, not OAuth.\nUse 'sparrow auth add {}' instead.",
                provider,
                provider
            )
        }
    };

    let cid = client_id
        .or_else(|| std::env::var(&cid_env).ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No OAuth client id for '{}'.\nPass --client-id <id> or set {}.\nRegister an OAuth app with your provider to obtain one.",
                provider, cid_env
            )
        })?;

    println!(
        "Starting OAuth device flow for {} ({})...",
        def.label, provider
    );
    let (verification_uri, user_code, device_code) =
        OAuthFlow::start_device_flow(&device_ep, &token_ep, &cid, &scope).await?;
    println!("\n  1. Open: {}", verification_uri);
    println!("  2. Enter code: {}\n", user_code);
    println!("Waiting for authorization (up to 5 min)...");

    let token = OAuthFlow::poll_token(&token_ep, &cid, &device_code, 300).await?;
    auth.set(provider, sparrow::auth::Credential::api_key(token))?;
    println!("Authenticated. Credential stored for {}.", provider);
    Ok(())
}

/// How `run_task` resolves the conversation context for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SessionMode {
    /// Continue this directory's session (existing behaviour).
    #[default]
    Auto,
    /// Ignore any saved session — start clean.
    Fresh,
    /// Continue the most recently updated session from ANY surface.
    ContinueLast,
}

/// Per-invocation run options threaded from the CLI flags.
#[derive(Debug, Clone, Copy, Default)]
struct RunFlags {
    session_mode: SessionMode,
    /// Skip the pre-run quote confirmation (--yes, or non-interactive).
    assume_yes: bool,
}
