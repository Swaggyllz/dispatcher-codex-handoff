use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

use crate::AppState;

async fn get_telemetry(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.telemetry.get_stats().await {
        Ok(stats) => Json(stats),
        Err(e) => Json(serde_json::json!({
            "error": e.to_string()
        })),
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/telemetry", get(get_telemetry))
}
