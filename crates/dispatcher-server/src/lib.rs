pub mod handoff;
pub mod protocol;
pub mod routes;
pub mod telemetry;

use axum::Router;
use dispatcher_engine::{
    ChatCompletionResponse, ModelRequest, Provider, ProviderError, RoutingConfig, RoutingEngine,
    StreamChunk,
};
use dispatcher_providers::ProviderRegistry;
use std::{path::PathBuf, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

pub struct AppState {
    pub engine: RoutingEngine,
    pub registry: ProviderRegistry,
    pub telemetry: telemetry::TelemetryStore,
    pub routing_config: RoutingConfig,
    pub policy_config_path: Option<PathBuf>,
}

fn provider_attempt_timeout_secs(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(1, 300)
}

pub(crate) fn provider_attempt_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(provider_attempt_timeout_secs(
        std::env::var("DISPATCHER_PROVIDER_TIMEOUT_SECS")
            .ok()
            .as_deref(),
    ))
}

fn bind_address(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("127.0.0.1")
}

pub(crate) async fn chat_completion_with_timeout(
    provider: &Arc<dyn Provider>,
    request: &ModelRequest,
    model_id: &str,
) -> Result<ChatCompletionResponse, ProviderError> {
    tokio::time::timeout(
        provider_attempt_timeout(),
        provider.chat_completion(request, model_id),
    )
    .await
    .map_err(|_| {
        ProviderError::Timeout(format!(
            "provider attempt exceeded {} seconds",
            provider_attempt_timeout().as_secs()
        ))
    })?
}

pub(crate) async fn chat_completion_stream_with_timeout(
    provider: &Arc<dyn Provider>,
    request: &ModelRequest,
    model_id: &str,
) -> Result<
    Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    ProviderError,
> {
    tokio::time::timeout(
        provider_attempt_timeout(),
        provider.chat_completion_stream(request, model_id),
    )
    .await
    .map_err(|_| {
        ProviderError::Timeout(format!(
            "stream connection exceeded {} seconds",
            provider_attempt_timeout().as_secs()
        ))
    })?
}

/// 启动 HTTP server
pub async fn run(port: u16, web_dir: Option<String>) -> anyhow::Result<()> {
    run_with_config(port, web_dir, RoutingConfig::default()).await
}

pub async fn run_with_config(
    port: u16,
    web_dir: Option<String>,
    routing_config: RoutingConfig,
) -> anyhow::Result<()> {
    run_with_config_path(port, web_dir, routing_config, None).await
}

pub async fn run_with_config_path(
    port: u16,
    web_dir: Option<String>,
    routing_config: RoutingConfig,
    policy_config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let engine = RoutingEngine::new(routing_config.clone());
    let registry = ProviderRegistry::from_env();
    let telemetry = telemetry::TelemetryStore::new("dispatcher_telemetry.db").await?;

    tracing::info!(
        "Provider attempt timeout: {}s (override with DISPATCHER_PROVIDER_TIMEOUT_SECS)",
        provider_attempt_timeout().as_secs()
    );
    for capability in registry.capabilities() {
        tracing::info!(
            "Provider registered: {} ({}) models={} api_key={} streaming={} tools={} vision={}",
            capability.provider_name,
            capability.provider_id,
            capability.supported_models.len(),
            capability.requires_api_key,
            capability.supports_streaming,
            capability.supports_tools,
            capability.supports_vision,
        );
    }
    if registry.capabilities().is_empty() {
        tracing::warn!("No providers registered; enable Demo or configure a provider API key");
    }

    let state = Arc::new(AppState {
        engine,
        registry,
        telemetry,
        routing_config,
        policy_config_path,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .merge(routes::chat::routes())
        .merge(routes::health::routes())
        .merge(routes::messages::routes())
        .merge(routes::models::routes())
        .merge(routes::policy::routes())
        .merge(routes::providers::routes())
        .merge(routes::responses::routes())
        .merge(routes::telemetry_route::routes());

    let mut app = Router::new()
        .nest("/v1", api_routes)
        .layer(cors)
        .with_state(state);

    // 如果指定了 web 目录，提供静态文件服务
    if let Some(dir) = web_dir {
        app = app.fallback_service(ServeDir::new(&dir));
        tracing::info!("Serving web dashboard from: {}", dir);
    }

    let bind_host = std::env::var("DISPATCHER_BIND_ADDR").ok();
    let addr = format!("{}:{}", bind_address(bind_host.as_deref()), port);
    tracing::info!("Dispatcher server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_timeout_uses_default_for_invalid_values_and_clamps_extremes() {
        assert_eq!(provider_attempt_timeout_secs(None), 30);
        assert_eq!(provider_attempt_timeout_secs(Some("invalid")), 30);
        assert_eq!(provider_attempt_timeout_secs(Some("0")), 1);
        assert_eq!(provider_attempt_timeout_secs(Some("500")), 300);
        assert_eq!(provider_attempt_timeout_secs(Some("12")), 12);
    }

    #[test]
    fn server_binds_to_loopback_by_default() {
        assert_eq!(bind_address(None), "127.0.0.1");
        assert_eq!(bind_address(Some("")), "127.0.0.1");
        assert_eq!(bind_address(Some("0.0.0.0")), "0.0.0.0");
    }
}
