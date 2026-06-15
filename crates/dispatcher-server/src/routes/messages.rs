use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Sse},
    routing::post,
    Json, Router,
};
use dispatcher_engine::types::*;
use futures::StreamExt;
use std::convert::Infallible;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::protocol::{self, AnthropicRequest};
use crate::{chat_completion_stream_with_timeout, chat_completion_with_timeout, AppState};

/// Anthropic Messages API — POST /v1/messages
async fn messages(
    State(state): State<Arc<AppState>>,
    Json(anthropic_req): Json<AnthropicRequest>,
) -> axum::response::Response {
    let is_stream = anthropic_req.stream;
    let openai_req = protocol::anthropic_to_openai(&anthropic_req);
    let capabilities = state.registry.capabilities().to_vec();
    let strategy = RoutingStrategy::Auto;
    let provider_health = state
        .telemetry
        .get_provider_health()
        .await
        .unwrap_or_default();

    let decision = match state
        .engine
        .route_with_health(&openai_req, &capabilities, strategy, &provider_health)
        .await
    {
        Some(d) => d,
        None => {
            return Json(serde_json::json!({
                "type": "error", "error": {"type": "no_provider", "message": "No available provider"}
            })).into_response();
        }
    };

    tracing::info!(
        "Anthropic route: {} -> {} via {} (stream={})",
        anthropic_req.model,
        decision.model_id,
        decision.provider_id,
        is_stream,
    );

    let provider = match state.registry.get(&decision.provider_id) {
        Some(p) => p.clone(),
        None => {
            return Json(serde_json::json!({
                "type": "error", "error": {"type": "not_found", "message": format!("Provider {} not found", decision.provider_id)}
            })).into_response();
        }
    };

    let provider_id = decision.provider_id.clone();
    let model_id = decision.model_id.clone();

    if is_stream {
        handle_anthropic_stream(
            state,
            provider,
            &provider_id,
            &model_id,
            &openai_req,
            &decision,
        )
        .await
    } else {
        handle_anthropic_non_stream(
            state,
            provider,
            &provider_id,
            &model_id,
            &openai_req,
            &decision,
            &anthropic_req.model,
        )
        .await
    }
}

/// 非流式 Anthropic 响应
async fn handle_anthropic_non_stream(
    state: Arc<AppState>,
    provider: Arc<dyn Provider>,
    provider_id: &str,
    model_id: &str,
    request: &ModelRequest,
    decision: &RoutingDecision,
    original_model: &str,
) -> axum::response::Response {
    match chat_completion_with_timeout(&provider, request, model_id).await {
        Ok(response) => {
            let mut served_decision = decision.clone();
            served_decision.fallback_chain.push(route_attempt(
                decision,
                RouteAttemptStatus::Success,
                None,
            ));
            record_telemetry(&state, &served_decision, &response, true, None).await;
            state
                .engine
                .circuit_breaker
                .record_success(provider_id)
                .await;

            let anthropic_resp = protocol::openai_to_anthropic(&response, original_model);
            Json(attach_routing(anthropic_resp, &served_decision)).into_response()
        }
        Err(e) => {
            state
                .engine
                .circuit_breaker
                .record_failure(provider_id)
                .await;

            record_telemetry(
                &state,
                decision,
                &failed_response(provider_id, model_id),
                false,
                Some(e.to_string()),
            )
            .await;
            let mut failed_decision = decision.clone();
            failed_decision.fallback_chain.push(route_attempt(
                decision,
                RouteAttemptStatus::Failed,
                Some(e.to_string()),
            ));

            let fallback_scores = decision
                .candidates
                .iter()
                .filter(|score| score.provider_id != provider_id)
                .cloned()
                .collect();

            let mut fallback_chain = failed_decision.fallback_chain.clone();
            for mut fallback in state
                .engine
                .selector
                .get_fallback_candidates(&failed_decision, fallback_scores)
            {
                fallback.fallback_chain = fallback_chain.clone();
                let Some(fallback_provider) = state.registry.get(&fallback.provider_id).cloned()
                else {
                    fallback_chain.push(route_attempt(
                        &fallback,
                        RouteAttemptStatus::Failed,
                        Some("provider not found".into()),
                    ));
                    continue;
                };

                match chat_completion_with_timeout(&fallback_provider, request, &fallback.model_id)
                    .await
                {
                    Ok(response) => {
                        let mut served_fallback = fallback.clone();
                        served_fallback.fallback_chain.push(route_attempt(
                            &fallback,
                            RouteAttemptStatus::Success,
                            None,
                        ));
                        record_telemetry(&state, &served_fallback, &response, true, None).await;
                        state
                            .engine
                            .circuit_breaker
                            .record_success(&fallback.provider_id)
                            .await;
                        let anthropic_resp =
                            protocol::openai_to_anthropic(&response, original_model);
                        return Json(attach_routing(anthropic_resp, &served_fallback))
                            .into_response();
                    }
                    Err(fallback_error) => {
                        record_telemetry(
                            &state,
                            &fallback,
                            &failed_response(&fallback.provider_id, &fallback.model_id),
                            false,
                            Some(fallback_error.to_string()),
                        )
                        .await;
                        state
                            .engine
                            .circuit_breaker
                            .record_failure(&fallback.provider_id)
                            .await;
                        fallback_chain.push(route_attempt(
                            &fallback,
                            RouteAttemptStatus::Failed,
                            Some(fallback_error.to_string()),
                        ));
                    }
                }
            }

            Json(serde_json::json!({
                "type": "error",
                "error": {"type": "provider_error", "message": e.to_string()},
                "routing": {"fallback_chain": fallback_chain},
            }))
            .into_response()
        }
    }
}

async fn record_telemetry(
    state: &Arc<AppState>,
    decision: &RoutingDecision,
    response: &ChatCompletionResponse,
    success: bool,
    error_message: Option<String>,
) {
    let _ = state
        .telemetry
        .record(&TelemetryRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            provider_id: decision.provider_id.clone(),
            model_id: decision.model_id.clone(),
            request_tokens: response.usage.prompt_tokens,
            response_tokens: response.usage.completion_tokens,
            latency_ms: response.latency_ms,
            cost_usd: estimate_cost_usd(decision, &response.usage),
            success,
            error_message,
            routing_strategy: format!("{:?}", decision.strategy),
            agent_tier: format!("{:?}", decision.agent_tier).to_lowercase(),
            is_fallback: decision.is_fallback,
        })
        .await;
}

fn estimate_cost_usd(decision: &RoutingDecision, usage: &Usage) -> f64 {
    decision
        .candidates
        .iter()
        .find(|candidate| {
            candidate.provider_id == decision.provider_id && candidate.model_id == decision.model_id
        })
        .map(|candidate| {
            (usage.prompt_tokens as f64 / 1000.0) * candidate.input_cost_per_1k
                + (usage.completion_tokens as f64 / 1000.0) * candidate.output_cost_per_1k
        })
        .unwrap_or(0.0)
}

fn failed_response(provider_id: &str, model_id: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: uuid::Uuid::new_v4().to_string(),
        model: model_id.to_string(),
        provider: provider_id.to_string(),
        choices: Vec::new(),
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
        finish_reason: None,
        latency_ms: 0,
    }
}

/// 流式 Anthropic SSE 响应
async fn handle_anthropic_stream(
    state: Arc<AppState>,
    provider: Arc<dyn Provider>,
    provider_id: &str,
    model_id: &str,
    request: &ModelRequest,
    decision: &RoutingDecision,
) -> axum::response::Response {
    match chat_completion_stream_with_timeout(&provider, request, model_id).await {
        Ok(stream) => anthropic_stream_response(state, stream, decision.clone()),
        Err(e) => {
            state
                .engine
                .circuit_breaker
                .record_failure(provider_id)
                .await;
            record_telemetry(
                &state,
                decision,
                &failed_response(provider_id, model_id),
                false,
                Some(e.to_string()),
            )
            .await;

            let mut failed_decision = decision.clone();
            failed_decision.fallback_chain.push(route_attempt(
                decision,
                RouteAttemptStatus::Failed,
                Some(e.to_string()),
            ));
            let fallback_scores = decision
                .candidates
                .iter()
                .filter(|score| score.provider_id != provider_id)
                .cloned()
                .collect();

            let mut fallback_chain = failed_decision.fallback_chain.clone();
            for mut fallback in state
                .engine
                .selector
                .get_fallback_candidates(&failed_decision, fallback_scores)
            {
                fallback.fallback_chain = fallback_chain.clone();
                let Some(fallback_provider) = state.registry.get(&fallback.provider_id).cloned()
                else {
                    fallback_chain.push(route_attempt(
                        &fallback,
                        RouteAttemptStatus::Failed,
                        Some("provider not found".into()),
                    ));
                    continue;
                };

                match chat_completion_stream_with_timeout(
                    &fallback_provider,
                    request,
                    &fallback.model_id,
                )
                .await
                {
                    Ok(stream) => {
                        let mut served_fallback = fallback.clone();
                        served_fallback.fallback_chain.push(route_attempt(
                            &fallback,
                            RouteAttemptStatus::Success,
                            None,
                        ));
                        return anthropic_stream_response(state, stream, served_fallback);
                    }
                    Err(fallback_error) => {
                        state
                            .engine
                            .circuit_breaker
                            .record_failure(&fallback.provider_id)
                            .await;
                        record_telemetry(
                            &state,
                            &fallback,
                            &failed_response(&fallback.provider_id, &fallback.model_id),
                            false,
                            Some(fallback_error.to_string()),
                        )
                        .await;
                        fallback_chain.push(route_attempt(
                            &fallback,
                            RouteAttemptStatus::Failed,
                            Some(fallback_error.to_string()),
                        ));
                    }
                }
            }

            Sse::new(futures::stream::once(async move {
                Ok::<Event, Infallible>(Event::default().data(
                    serde_json::json!({"type":"error","error":{"type":"provider_error","message":e.to_string()}}).to_string(),
                ))
            })).into_response()
        }
    }
}

fn anthropic_stream_response(
    state: Arc<AppState>,
    stream: Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    decision: RoutingDecision,
) -> axum::response::Response {
    let pid = decision.provider_id.clone();
    let started = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    let state_for_errors = state.clone();
    let error_pid = pid.clone();
    let started_for_chunks = started.clone();
    let stopped_for_chunks = stopped.clone();
    let sse_stream = stream.flat_map(move |chunk_result| {
        let events: Vec<Result<Event, Infallible>> = match chunk_result {
            Ok(chunk) => protocol::stream_chunk_to_sse_json(
                &chunk,
                &started_for_chunks,
                &stopped_for_chunks,
            )
                .into_iter()
                .map(|payload| {
                    Ok(Event::default()
                        .event(payload.event_type)
                        .data(payload.json))
                })
                .collect(),
            Err(e) => {
                let state = state_for_errors.clone();
                let provider_id = error_pid.clone();
                tokio::spawn(async move {
                    state
                        .engine
                        .circuit_breaker
                        .record_failure(&provider_id)
                        .await;
                });
                vec![Ok(Event::default().data(
                    serde_json::json!({"type":"error","error":{"type":"provider_error","message":e.to_string()}}).to_string(),
                ))]
            }
        };
        futures::stream::iter(events)
    });
    let terminal_events =
        futures::stream::once(async move { protocol::stream_end_to_sse_json(&started, &stopped) })
            .flat_map(|events| {
                futures::stream::iter(events.into_iter().map(|payload| {
                    Ok::<Event, Infallible>(
                        Event::default()
                            .event(payload.event_type)
                            .data(payload.json),
                    )
                }))
            });

    tokio::spawn(async move {
        state.engine.circuit_breaker.record_success(&pid).await;
    });

    Sse::new(sse_stream.chain(terminal_events)).into_response()
}

fn route_attempt(
    decision: &RoutingDecision,
    status: RouteAttemptStatus,
    error: Option<String>,
) -> RouteAttempt {
    RouteAttempt {
        provider_id: decision.provider_id.clone(),
        model_id: decision.model_id.clone(),
        status,
        error,
    }
}

fn attach_routing(
    mut response: serde_json::Value,
    decision: &RoutingDecision,
) -> serde_json::Value {
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "routing".into(),
            serde_json::json!({
                "provider": decision.provider_id,
                "strategy": format!("{:?}", decision.strategy),
                "agent_tier": decision.agent_tier,
                "is_fallback": decision.is_fallback,
                "fallback_reason": decision.fallback_reason,
                "fallback_chain": decision.fallback_chain,
                "policy_reason": decision.policy_reason,
                "decision_reason": decision.decision_reason,
                "decision_time_ms": decision.decision_time_ms,
            }),
        );
    }
    response
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/messages", post(messages))
}
