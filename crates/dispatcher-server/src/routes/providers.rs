use axum::{extract::State, routing::get, Json, Router};
use dispatcher_engine::{CircuitBreakerSnapshot, CircuitBreakerState, ProviderHealthSnapshot};
use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

async fn list_providers(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let capabilities = state.registry.capabilities();
    let health = state
        .telemetry
        .get_provider_health()
        .await
        .unwrap_or_default();
    let circuit_snapshots: HashMap<_, _> = state
        .engine
        .circuit_breaker
        .snapshots()
        .await
        .into_iter()
        .map(|snapshot| (snapshot.provider_id.clone(), snapshot))
        .collect();
    let providers: Vec<_> = capabilities
        .iter()
        .map(|cap| {
            let provider_health = health.get(&cap.provider_id);
            let circuit = circuit_snapshots.get(&cap.provider_id);
            serde_json::json!({
                "id": cap.provider_id,
                "name": cap.provider_name,
                "models": cap.supported_models.iter().map(|m| {
                    serde_json::json!({
                        "id": m.model_id,
                        "name": m.display_name,
                        "cost": {
                            "input_per_1k": m.input_cost_per_1k,
                            "output_per_1k": m.output_cost_per_1k,
                        },
                        "pricing_source": m.pricing_source,
                        "pricing_updated_at": m.pricing_updated_at,
                        "supports_streaming": cap.supports_streaming
                            && m.supports_streaming.unwrap_or(true),
                        "supports_tools": cap.supports_tools && m.supports_tools.unwrap_or(true),
                        "supports_vision": cap.supports_vision && m.supports_vision.unwrap_or(true),
                        "max_tokens": m.max_tokens,
                        "quality_score": m.quality_score,
                        "avg_latency_ms": m.avg_latency_ms,
                        "handoff_certification": m.handoff_certification,
                    })
                }).collect::<Vec<_>>(),
                "supports_streaming": cap.supports_streaming,
                "supports_tools": cap.supports_tools,
                "supports_vision": cap.supports_vision,
                "health": {
                    "status": provider_health_status(provider_health, circuit),
                    "sample_count": provider_health.map(|snapshot| snapshot.sample_count).unwrap_or(0),
                    "success_rate": provider_health.map(|snapshot| snapshot.success_rate),
                    "avg_latency_ms": provider_health.map(|snapshot| snapshot.avg_latency_ms),
                    "circuit_state": circuit.map(|snapshot| snapshot.state).unwrap_or(CircuitBreakerState::Closed),
                    "failure_count": circuit.map(|snapshot| snapshot.failure_count).unwrap_or(0),
                    "cooldown_remaining_secs": circuit.map(|snapshot| snapshot.cooldown_remaining_secs).unwrap_or(0),
                },
            })
        })
        .collect();

    Json(serde_json::json!({ "providers": providers }))
}

fn provider_health_status(
    health: Option<&ProviderHealthSnapshot>,
    circuit: Option<&CircuitBreakerSnapshot>,
) -> &'static str {
    if let Some(circuit) = circuit {
        match circuit.state {
            CircuitBreakerState::Open => return "down",
            CircuitBreakerState::HalfOpen => return "degraded",
            CircuitBreakerState::Closed => {}
        }
    }

    match health {
        Some(snapshot) if snapshot.sample_count >= 3 && snapshot.success_rate >= 0.9 => "healthy",
        Some(snapshot) if snapshot.sample_count >= 3 => "degraded",
        _ => "unknown",
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/providers", get(list_providers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use dispatcher_engine::{RoutingConfig, RoutingEngine};
    use dispatcher_providers::ProviderRegistry;
    use std::path::PathBuf;

    #[test]
    fn open_circuit_is_reported_as_down() {
        let health = ProviderHealthSnapshot {
            provider_id: "alpha".into(),
            sample_count: 20,
            success_rate: 0.99,
            avg_latency_ms: 500,
        };
        let circuit = CircuitBreakerSnapshot {
            provider_id: "alpha".into(),
            state: CircuitBreakerState::Open,
            failure_count: 3,
            cooldown_remaining_secs: 20,
        };

        assert_eq!(
            provider_health_status(Some(&health), Some(&circuit)),
            "down"
        );
    }

    #[test]
    fn poor_recent_success_rate_is_reported_as_degraded() {
        let health = ProviderHealthSnapshot {
            provider_id: "alpha".into(),
            sample_count: 10,
            success_rate: 0.7,
            avg_latency_ms: 500,
        };

        assert_eq!(provider_health_status(Some(&health), None), "degraded");
    }

    #[tokio::test]
    async fn providers_response_includes_model_metadata_source_and_effective_capabilities() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-provider-route-metadata-{}.toml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
[[providers]]
id = "demo"

[[providers.models]]
id = "demo-echo"
pricing_source = "bundled-test"
pricing_updated_at = "2026-06-08"
supports_tools = false
supports_vision = false
handoff_certification = { labels = ["handoff_text_only"], eval_set = "dispatcher-handoff-v0.3.0-fixtures", evaluated_at = "2026-06-18" }
"#,
        )
        .unwrap();

        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("SILICONFLOW_API_KEY");
        std::env::remove_var("MIMO_API_KEY");
        std::env::remove_var("XIAOMIMIMO_API_KEY");
        std::env::remove_var("DISPATCHER_DEMO_PROVIDER");
        std::env::set_var("DISPATCHER_PROVIDER_METADATA", &path);

        let db_path = std::env::temp_dir().join(format!(
            "dispatcher-provider-route-metadata-{}.db",
            uuid::Uuid::new_v4()
        ));
        let state = Arc::new(crate::AppState {
            engine: RoutingEngine::new(RoutingConfig::default()),
            registry: ProviderRegistry::from_env(),
            telemetry: crate::telemetry::TelemetryStore::new(db_path.to_string_lossy().as_ref())
                .await
                .unwrap(),
            routing_config: RoutingConfig::default(),
            policy_config_path: Option::<PathBuf>::None,
        });

        let Json(body) = list_providers(State(state)).await;
        let demo = body["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|provider| provider["id"] == "demo")
            .unwrap();
        let model = demo["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["id"] == "demo-echo")
            .unwrap();

        std::env::remove_var("DISPATCHER_PROVIDER_METADATA");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(db_path).unwrap();

        assert_eq!(model["pricing_source"], "bundled-test");
        assert_eq!(model["pricing_updated_at"], "2026-06-08");
        assert_eq!(model["supports_tools"], false);
        assert_eq!(model["supports_vision"], false);
        assert_eq!(model["supports_streaming"], true);
        assert_eq!(
            model["handoff_certification"]["labels"][0],
            "handoff_text_only"
        );
    }
}
