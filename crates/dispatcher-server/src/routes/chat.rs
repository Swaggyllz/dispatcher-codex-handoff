use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::post,
    Json, Router,
};
use dispatcher_engine::types::*;
use futures::StreamExt;
use std::sync::Arc;

use crate::{chat_completion_stream_with_timeout, chat_completion_with_timeout, AppState};

/// OpenAI-compatible chat completions — 自动处理 stream: true/false
async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ModelRequest>,
) -> axum::response::Response {
    let is_stream = request.stream;

    let capabilities = state.registry.capabilities().to_vec();
    let strategy = request
        .extra
        .get("strategy")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "save" => RoutingStrategy::Save,
            "fast" => RoutingStrategy::Fast,
            _ => RoutingStrategy::Auto,
        })
        .unwrap_or(RoutingStrategy::Auto);

    let provider_health = state
        .telemetry
        .get_provider_health()
        .await
        .unwrap_or_default();
    let decision = match state
        .engine
        .route_with_health(&request, &capabilities, strategy, &provider_health)
        .await
    {
        Some(d) => d,
        None => {
            return Json(serde_json::json!({
                "error": {"message": "No available provider", "type": "no_provider_available"}
            }))
            .into_response();
        }
    };

    tracing::info!(
        "Routing: {} -> {} via {} (score={:.3}, {}ms, stream={})",
        request.model,
        decision.model_id,
        decision.provider_id,
        decision
            .candidates
            .first()
            .map(|s| s.total_score)
            .unwrap_or(0.0),
        decision.decision_time_ms,
        is_stream,
    );

    let provider = match state.registry.get(&decision.provider_id) {
        Some(p) => p.clone(),
        None => {
            return Json(serde_json::json!({
                "error": {"message": format!("Provider {} not found", decision.provider_id), "type": "provider_not_found"}
            }))
            .into_response();
        }
    };
    let provider_id = decision.provider_id.clone();
    let model_id = decision.model_id.clone();

    if is_stream {
        handle_stream(
            state,
            provider,
            &provider_id,
            &model_id,
            &request,
            &decision,
        )
        .await
    } else {
        handle_non_stream(
            state,
            provider,
            &provider_id,
            &model_id,
            &request,
            &decision,
        )
        .await
    }
}

/// 非流式响应
async fn handle_non_stream(
    state: Arc<AppState>,
    provider: Arc<dyn Provider>,
    provider_id: &str,
    model_id: &str,
    request: &ModelRequest,
    decision: &RoutingDecision,
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

            Json(chat_response_json(&response, &served_decision)).into_response()
        }
        Err(e) => {
            state
                .engine
                .circuit_breaker
                .record_failure(provider_id)
                .await;

            let failed_primary_response = failed_response(provider_id, model_id);
            record_telemetry(
                &state,
                decision,
                &failed_primary_response,
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
                        return Json(chat_response_json(&response, &served_fallback))
                            .into_response();
                    }
                    Err(fallback_error) => {
                        let fallback_failed =
                            failed_response(&fallback.provider_id, &fallback.model_id);
                        record_telemetry(
                            &state,
                            &fallback,
                            &fallback_failed,
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
                "error": {"message": format!("Provider error: {}", e), "type": "provider_error"},
                "routing": {"fallback_chain": fallback_chain},
            }))
            .into_response()
        }
    }
}

fn chat_response_json(
    response: &ChatCompletionResponse,
    decision: &RoutingDecision,
) -> serde_json::Value {
    let choice = response.choices.first();
    let top_candidates: Vec<_> = decision
        .candidates
        .iter()
        .take(5)
        .map(|candidate| {
            serde_json::json!({
                "provider": candidate.provider_id,
                "model": candidate.model_id,
                "total_score": candidate.total_score,
                "quality_score": candidate.quality_score,
                "cost_score": candidate.cost_score,
                "latency_score": candidate.latency_score,
                "availability_score": candidate.availability_score,
                "availability": candidate.availability,
                "estimated_cost_per_1k": candidate.estimated_cost_per_1k,
                "input_cost_per_1k": candidate.input_cost_per_1k,
                "output_cost_per_1k": candidate.output_cost_per_1k,
                "avg_latency_ms": candidate.avg_latency_ms,
                "policy_reason": candidate.policy_reason,
                "score_breakdown": candidate.score_breakdown,
            })
        })
        .collect();

    serde_json::json!({
        "id": response.id,
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": response.model,
        "provider": response.provider,
        "choices": [{
            "index": choice.map(|c| c.index).unwrap_or(0),
            "message": {
                "role": choice.map(|c| c.message.role.as_str()).unwrap_or("assistant"),
                "content": choice.map(|c| c.message.content.as_str()).unwrap_or(""),
            },
            "finish_reason": choice.and_then(|c| c.finish_reason.as_deref()),
        }],
        "usage": {
            "prompt_tokens": response.usage.prompt_tokens,
            "completion_tokens": response.usage.completion_tokens,
            "total_tokens": response.usage.total_tokens,
        },
        "routing": {
            "provider": decision.provider_id,
            "strategy": format!("{:?}", decision.strategy),
            "agent_tier": decision.agent_tier,
            "is_fallback": decision.is_fallback,
            "fallback_reason": decision.fallback_reason,
            "fallback_chain": decision.fallback_chain,
            "policy_reason": decision.policy_reason,
            "decision_reason": decision.decision_reason,
            "top_candidates": top_candidates,
            "excluded_candidates": decision.excluded_candidates,
            "decision_time_ms": decision.decision_time_ms,
        }
    })
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

/// SSE 流式响应
async fn handle_stream(
    state: Arc<AppState>,
    provider: Arc<dyn Provider>,
    provider_id: &str,
    model_id: &str,
    request: &ModelRequest,
    decision: &RoutingDecision,
) -> axum::response::Response {
    use std::convert::Infallible;

    match chat_completion_stream_with_timeout(&provider, request, model_id).await {
        Ok(stream) => {
            let mut served_decision = decision.clone();
            served_decision.fallback_chain.push(route_attempt(
                decision,
                RouteAttemptStatus::Success,
                None,
            ));
            openai_stream_response(state, stream, served_decision)
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
                        return openai_stream_response(state, stream, served_fallback);
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

            let error_json = serde_json::json!({
                "error": {"message": format!("Provider error: {}", e), "type": "provider_error"},
                "routing": {"fallback_chain": fallback_chain},
            });
            Sse::new(futures::stream::once(async move {
                Ok::<Event, Infallible>(Event::default().data(error_json.to_string()))
            }))
            .into_response()
        }
    }
}

fn openai_stream_response(
    state: Arc<AppState>,
    stream: Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    decision: RoutingDecision,
) -> axum::response::Response {
    use std::convert::Infallible;

    let pid = decision.provider_id.clone();
    let strategy = format!("{:?}", decision.strategy);
    let agent_tier = serde_json::json!(decision.agent_tier);
    let is_fallback = decision.is_fallback;
    let fallback_reason = decision.fallback_reason.clone();
    let fallback_chain = decision.fallback_chain.clone();
    let policy_reason = decision.policy_reason.clone();
    let decision_reason = decision.decision_reason.clone();
    let excluded_candidates = decision.excluded_candidates.clone();
    let decision_time = decision.decision_time_ms;
    let state_for_stream = state.clone();
    let state_for_errors = state.clone();
    let error_pid = pid.clone();

    let sse_stream = stream
        .map(move |chunk_result| match chunk_result {
            Ok(chunk) => {
                let json = serde_json::to_string(&chunk).unwrap_or_default();
                Ok::<Event, Infallible>(Event::default().data(json))
            }
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
                let json = serde_json::json!({"error": e.to_string()});
                Ok(Event::default().data(json.to_string()))
            }
        })
        .chain(futures::stream::once({
            let pid = pid.clone();
            async move {
                let meta = serde_json::json!({
                    "routing": {
                        "provider": pid,
                        "strategy": strategy,
                        "agent_tier": agent_tier,
                        "is_fallback": is_fallback,
                        "fallback_reason": fallback_reason,
                        "fallback_chain": fallback_chain,
                        "policy_reason": policy_reason,
                        "decision_reason": decision_reason,
                        "excluded_candidates": excluded_candidates,
                        "decision_time_ms": decision_time,
                    }
                });
                Ok::<Event, Infallible>(Event::default().data(meta.to_string()).event("routing"))
            }
        }))
        .chain(futures::stream::once(async {
            Ok::<Event, Infallible>(Event::default().data("[DONE]"))
        }));

    tokio::spawn(async move {
        state_for_stream
            .engine
            .circuit_breaker
            .record_success(&pid)
            .await;
    });

    Sse::new(sse_stream).into_response()
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/chat/completions", post(chat_completions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatcher_engine::{
        AgentTier, AvailabilityStatus, HandoffCertification, ProviderScore, RouteAttempt,
        RoutingStrategy, ScoreBreakdown,
    };

    fn decision_with_cost_per_1k(estimated_cost_per_1k: f64) -> RoutingDecision {
        RoutingDecision {
            provider_id: "alpha".into(),
            model_id: "alpha-model".into(),
            strategy: RoutingStrategy::Auto,
            agent_tier: AgentTier::Medium,
            candidates: vec![ProviderScore {
                provider_id: "alpha".into(),
                model_id: "alpha-model".into(),
                total_score: 0.8,
                quality_score: 0.8,
                cost_score: 0.7,
                latency_score: 0.6,
                availability_score: 1.0,
                estimated_cost_per_1k,
                input_cost_per_1k: 0.003,
                output_cost_per_1k: 0.009,
                avg_latency_ms: 500,
                availability: AvailabilityStatus::Available,
                policy_reason: None,
                handoff_certification: HandoffCertification::default(),
                score_breakdown: ScoreBreakdown {
                    weighted_quality: 0.4,
                    weighted_cost: 0.2,
                    weighted_latency: 0.1,
                    weighted_availability: 0.1,
                    weighted_policy: 0.0,
                },
            }],
            decision_time_ms: 1,
            is_fallback: false,
            fallback_reason: None,
            fallback_chain: Vec::<RouteAttempt>::new(),
            policy_reason: None,
            decision_reason: "test".into(),
            excluded_candidates: Vec::new(),
        }
    }

    #[test]
    fn estimate_cost_uses_input_and_output_prices_separately() {
        let decision = decision_with_cost_per_1k(0.012);
        let usage = Usage {
            prompt_tokens: 1000,
            completion_tokens: 1000,
            total_tokens: 2000,
        };

        assert!((estimate_cost_usd(&decision, &usage) - 0.012).abs() < 0.000001);
    }
}
