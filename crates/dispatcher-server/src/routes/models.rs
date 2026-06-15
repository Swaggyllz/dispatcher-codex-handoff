use axum::{extract::State, routing::get, Json, Router};
use dispatcher_providers::ProviderRegistry;
use std::sync::Arc;

use crate::AppState;

fn model_list(registry: &ProviderRegistry) -> serde_json::Value {
    let data = registry
        .capabilities()
        .iter()
        .flat_map(|provider| {
            provider.supported_models.iter().map(|model| {
                serde_json::json!({
                    "id": model.model_id,
                    "object": "model",
                    "created": 0,
                    "owned_by": provider.provider_id,
                })
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "object": "list",
        "data": data,
    })
}

async fn list_models(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(model_list(&state.registry))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/models", get(list_models))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatcher_providers::{demo::DemoProvider, ProviderRegistry};
    use std::sync::Arc;

    #[test]
    fn model_list_uses_openai_shape_and_provider_ownership() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(DemoProvider::new()));

        let response = model_list(&registry);
        let models = response["data"].as_array().unwrap();

        assert_eq!(response["object"], "list");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["id"], "demo-echo");
        assert_eq!(models[0]["object"], "model");
        assert_eq!(models[0]["owned_by"], "demo");
    }
}
