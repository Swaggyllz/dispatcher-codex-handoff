use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::post,
    Json, Router,
};
use dispatcher_engine::types::*;
use dispatcher_engine::RequestAnalyzer;
use dispatcher_providers::http_client::build_client;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use crate::routes::responses_compat::{
    chat_completion_to_response, ResponsesSseEvent, ResponsesStreamState,
};
use crate::{
    chat_completion_stream_with_timeout, chat_completion_with_timeout, provider_attempt_timeout,
    telemetry::CodexTelemetryRecord, AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponsesRequest {
    model: String,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    input: Vec<ResponseInputItem>,
    #[serde(default)]
    tools: Vec<ResponseTool>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    max_output_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    reasoning: Option<CodexReasoning>,
    #[serde(default)]
    service_tier: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CodexReasoning {
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponseInputItem {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Vec<ResponseContentPart>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<serde_json::Value>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponseContentPart {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponseTool {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexSpeed {
    Standard,
    Priority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexRoute {
    model: String,
    reasoning_effort: String,
    speed: CodexSpeed,
    agent_tier: AgentTier,
    reason: String,
}

struct CodexAuth {
    bearer_token: String,
    chatgpt_account_id: Option<String>,
}

enum CodexUpstreamSendError {
    Request(reqwest::Error),
    Timeout,
}

fn response_arguments_text(arguments: Option<&serde_json::Value>) -> String {
    match arguments {
        Some(serde_json::Value::String(arguments)) => arguments.clone(),
        Some(arguments) => arguments.to_string(),
        None => "{}".into(),
    }
}

fn codex_speed_label(speed: CodexSpeed) -> &'static str {
    match speed {
        CodexSpeed::Standard => "standard",
        CodexSpeed::Priority => "priority",
    }
}

fn codex_telemetry_record(
    requested_model: &str,
    route: &CodexRoute,
    success: bool,
    status_code: Option<StatusCode>,
    latency_ms: u64,
    error_message: Option<String>,
) -> CodexTelemetryRecord {
    CodexTelemetryRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        requested_model: requested_model.into(),
        model_id: route.model.clone(),
        reasoning_effort: route.reasoning_effort.clone(),
        speed: codex_speed_label(route.speed).into(),
        agent_tier: format!("{:?}", route.agent_tier).to_lowercase(),
        reason: route.reason.clone(),
        success,
        status_code: status_code.map(|status| status.as_u16()),
        latency_ms,
        error_message,
    }
}

async fn record_codex_outcome(
    state: &Arc<AppState>,
    requested_model: &str,
    route: &CodexRoute,
    success: bool,
    status_code: Option<StatusCode>,
    latency_ms: u64,
    error_message: Option<String>,
) {
    let record = codex_telemetry_record(
        requested_model,
        route,
        success,
        status_code,
        latency_ms,
        error_message,
    );
    if let Err(error) = state.telemetry.record_codex_route(&record).await {
        tracing::warn!("Failed to record Codex route telemetry: {error}");
    }
}

#[cfg(test)]
fn select_codex_route(request: &ResponsesRequest, model_request: &ModelRequest) -> CodexRoute {
    select_codex_route_for_mode(request, model_request, request.model == "dispatcher-auto")
}

fn select_codex_route_for_mode(
    request: &ResponsesRequest,
    model_request: &ModelRequest,
    dispatcher_auto: bool,
) -> CodexRoute {
    let features = RequestAnalyzer::analyze(model_request);
    let requested_model = request.model.as_str();
    let model =
        if !dispatcher_auto && matches!(requested_model, "gpt-5.5" | "gpt-5.4" | "gpt-5.4-mini") {
            requested_model
        } else {
            match features.agent_tier {
                AgentTier::Simple => "gpt-5.4-mini",
                AgentTier::Medium => "gpt-5.4",
                AgentTier::Reasoning | AgentTier::Complex => "gpt-5.5",
            }
        };

    let default_effort = match features.agent_tier {
        AgentTier::Simple => "low",
        AgentTier::Medium => "medium",
        AgentTier::Reasoning => "high",
        AgentTier::Complex => "xhigh",
    };
    let reasoning_effort = if dispatcher_auto {
        default_effort
    } else {
        request
            .reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.effort.as_deref())
            .filter(|effort| matches!(*effort, "low" | "medium" | "high" | "xhigh"))
            .unwrap_or(default_effort)
    };

    let wants_priority = request.service_tier.as_deref() == Some("priority");
    let priority_requested = if dispatcher_auto {
        features.agent_tier == AgentTier::Medium
    } else {
        wants_priority
    };
    let speed = if priority_requested && model != "gpt-5.4-mini" {
        CodexSpeed::Priority
    } else {
        CodexSpeed::Standard
    };

    CodexRoute {
        model: model.into(),
        reasoning_effort: reasoning_effort.into(),
        speed,
        agent_tier: features.agent_tier,
        reason: format!(
            "{:?} task -> {} with {} reasoning and {} speed",
            features.agent_tier,
            model,
            reasoning_effort,
            codex_speed_label(speed)
        ),
    }
}

fn build_codex_upstream_body(
    mut raw_request: serde_json::Value,
    route: &CodexRoute,
) -> serde_json::Value {
    let Some(object) = raw_request.as_object_mut() else {
        return raw_request;
    };

    object.insert("model".into(), serde_json::json!(route.model));
    let reasoning = object
        .entry("reasoning")
        .or_insert_with(|| serde_json::json!({}));
    if !reasoning.is_object() {
        *reasoning = serde_json::json!({});
    }
    reasoning["effort"] = serde_json::json!(route.reasoning_effort);
    object.insert(
        "service_tier".into(),
        serde_json::json!(match route.speed {
            CodexSpeed::Standard => "auto",
            CodexSpeed::Priority => "priority",
        }),
    );

    raw_request
}

fn prepare_codex_upstream_body_for_auth(
    mut upstream_body: serde_json::Value,
    route: &CodexRoute,
    auth: &CodexAuth,
) -> serde_json::Value {
    if auth.chatgpt_account_id.is_none() {
        return upstream_body;
    }
    let Some(object) = upstream_body.as_object_mut() else {
        return upstream_body;
    };

    match route.speed {
        CodexSpeed::Standard => {
            object.remove("service_tier");
        }
        CodexSpeed::Priority => {
            object.insert("service_tier".into(), serde_json::json!("fast"));
        }
    }

    upstream_body
}

fn should_retry_chatgpt_without_fast(
    status: StatusCode,
    route: &CodexRoute,
    auth: &CodexAuth,
) -> bool {
    status == StatusCode::BAD_REQUEST
        && route.speed == CodexSpeed::Priority
        && auth.chatgpt_account_id.is_some()
}

fn without_service_tier(mut upstream_body: serde_json::Value) -> serde_json::Value {
    if let Some(object) = upstream_body.as_object_mut() {
        object.remove("service_tier");
    }
    upstream_body
}

fn codex_upstream_url(base_url: Option<&str>) -> String {
    format!(
        "{}/responses",
        base_url
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/')
    )
}

fn codex_upstream_url_for_auth(base_url: Option<&str>, auth: &CodexAuth) -> String {
    let default_base = if auth.chatgpt_account_id.is_some() {
        "https://chatgpt.com/backend-api/codex"
    } else {
        "https://api.openai.com/v1"
    };
    codex_upstream_url(
        base_url
            .filter(|value| !value.trim().is_empty())
            .or(Some(default_base)),
    )
}

fn codex_api_key_from(dedicated_key: Option<&str>, openai_key: Option<&str>) -> Option<String> {
    dedicated_key
        .filter(|key| !key.is_empty())
        .or_else(|| openai_key.filter(|key| !key.is_empty()))
        .map(str::to_string)
}

fn codex_auth_from(
    dedicated_key: Option<&str>,
    openai_key: Option<&str>,
    authorization: Option<&str>,
    chatgpt_account_id: Option<&str>,
) -> Option<CodexAuth> {
    let client_token = authorization
        .map(str::trim)
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim())
        .filter(|token| !token.is_empty() && *token != "local-dispatcher");
    let account_id = chatgpt_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let (Some(token), Some(account_id)) = (client_token, account_id) {
        return Some(CodexAuth {
            bearer_token: token.into(),
            chatgpt_account_id: Some(account_id.into()),
        });
    }

    if let Some(bearer_token) = codex_api_key_from(dedicated_key, openai_key) {
        return Some(CodexAuth {
            bearer_token,
            chatgpt_account_id: None,
        });
    }

    Some(CodexAuth {
        bearer_token: client_token?.into(),
        chatgpt_account_id: None,
    })
}

fn codex_auth(headers: &HeaderMap) -> Option<CodexAuth> {
    let dedicated_key = std::env::var("DISPATCHER_CODEX_API_KEY").ok();
    let openai_key = std::env::var("OPENAI_API_KEY").ok();
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let chatgpt_account_id = headers
        .get("ChatGPT-Account-Id")
        .and_then(|value| value.to_str().ok());
    codex_auth_from(
        dedicated_key.as_deref(),
        openai_key.as_deref(),
        authorization,
        chatgpt_account_id,
    )
}

fn dispatcher_auto_header(headers: &HeaderMap) -> bool {
    headers
        .get("X-Dispatcher-Mode")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("auto"))
}

fn provider_auto_header(headers: &HeaderMap) -> bool {
    headers
        .get("X-Dispatcher-Mode")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("provider-auto"))
}

fn unsupported_provider_tools(request: &ResponsesRequest) -> Vec<&str> {
    request
        .tools
        .iter()
        .filter(|tool| {
            !matches!(
                tool.tool_type.as_str(),
                "function" | "custom" | "tool_search"
            )
        })
        .map(|tool| tool.tool_type.as_str())
        .collect()
}

async fn send_codex_upstream(
    client: &reqwest::Client,
    url: &str,
    auth: &CodexAuth,
    upstream_body: &serde_json::Value,
    stream: bool,
) -> Result<reqwest::Response, CodexUpstreamSendError> {
    let mut request = client.post(url).bearer_auth(&auth.bearer_token).header(
        header::ACCEPT,
        if stream {
            "text/event-stream"
        } else {
            "application/json"
        },
    );
    if let Some(account_id) = auth.chatgpt_account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    match tokio::time::timeout(
        provider_attempt_timeout(),
        request.json(upstream_body).send(),
    )
    .await
    {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(CodexUpstreamSendError::Request(error)),
        Err(_) => Err(CodexUpstreamSendError::Timeout),
    }
}

fn responses_to_model_request(request: &ResponsesRequest) -> ModelRequest {
    let mut messages = Vec::new();

    if let Some(instructions) = request
        .instructions
        .as_deref()
        .filter(|instructions| !instructions.is_empty())
    {
        messages.push(Message {
            role: "system".into(),
            content: MessageContent::Text(instructions.into()),
        });
    }

    for item in &request.input {
        match item.item_type.as_str() {
            "message" => {
                let role = match item.role.as_deref().unwrap_or("user") {
                    "developer" => "system",
                    role => role,
                };
                let parts = item
                    .content
                    .iter()
                    .filter_map(|part| match part.content_type.as_str() {
                        "input_text" | "output_text" => {
                            part.text.as_ref().map(|text| ContentPart {
                                content_type: "text".into(),
                                text: Some(text.clone()),
                                image_url: None,
                            })
                        }
                        "input_image" => part.image_url.as_ref().map(|url| ContentPart {
                            content_type: "image_url".into(),
                            text: None,
                            image_url: Some(ImageUrl { url: url.clone() }),
                        }),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                if !parts.is_empty() {
                    messages.push(Message {
                        role: role.into(),
                        content: if parts.len() == 1
                            && parts[0].content_type == "text"
                            && parts[0].text.is_some()
                        {
                            MessageContent::Text(parts[0].text.clone().unwrap_or_default())
                        } else {
                            MessageContent::MultiPart(parts)
                        },
                    });
                }
            }
            "function_call" => {
                messages.push(Message {
                    role: "assistant".into(),
                    content: MessageContent::Text(format!(
                        "[Tool call {} id={}]\n{}",
                        item.name.as_deref().unwrap_or("unknown"),
                        item.call_id.as_deref().unwrap_or("unknown"),
                        response_arguments_text(item.arguments.as_ref())
                    )),
                });
            }
            "function_call_output" => {
                let output = item
                    .output
                    .as_ref()
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        value => value.to_string(),
                    })
                    .unwrap_or_default();
                messages.push(Message {
                    role: "user".into(),
                    content: MessageContent::Text(format!(
                        "[Tool result id={}]\n{}",
                        item.call_id.as_deref().unwrap_or("unknown"),
                        output
                    )),
                });
            }
            _ => {}
        }
    }

    let tools = request
        .tools
        .iter()
        .filter(|tool| tool.tool_type == "function" && !tool.name.is_empty())
        .map(|tool| Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        })
        .collect::<Vec<_>>();

    let extra = [
        "session_id",
        "sessionId",
        "conversation_id",
        "thread_id",
        "strategy",
    ]
    .into_iter()
    .filter_map(|key| {
        request
            .extra
            .get(key)
            .cloned()
            .map(|value| (key.into(), value))
    })
    .collect();

    ModelRequest {
        model: request.model.clone(),
        messages,
        temperature: request.temperature,
        max_tokens: request.max_output_tokens,
        stream: request.stream,
        tools: (!tools.is_empty()).then_some(tools),
        extra,
    }
}

async fn responses(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(raw_request): Json<serde_json::Value>,
) -> axum::response::Response {
    let request = match serde_json::from_value::<ResponsesRequest>(raw_request.clone()) {
        Ok(request) => request,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "message": format!("Invalid Responses API request: {error}"),
                        "type": "invalid_request"
                    }
                })),
            )
                .into_response();
        }
    };
    let model_request = responses_to_model_request(&request);
    if provider_auto_header(&headers) {
        let unsupported_tools = unsupported_provider_tools(&request);
        if !unsupported_tools.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "message": format!(
                            "provider-auto does not support hosted tools: {}",
                            unsupported_tools.join(", ")
                        ),
                        "type": "unsupported_provider_tool"
                    }
                })),
            )
                .into_response();
        }
        return provider_responses(state, request, model_request).await;
    }
    let route = select_codex_route_for_mode(
        &request,
        &model_request,
        request.model == "dispatcher-auto" || dispatcher_auto_header(&headers),
    );
    let upstream_body = build_codex_upstream_body(raw_request, &route);

    tracing::info!(
        "Codex route: {} -> {} effort={} speed={:?} tier={:?} (stream={})",
        request.model,
        route.model,
        route.reasoning_effort,
        route.speed,
        route.agent_tier,
        request.stream,
    );

    forward_codex_response(
        &state,
        request.model.as_str(),
        upstream_body,
        &route,
        request.stream,
        codex_auth(&headers),
    )
    .await
}

fn provider_strategy(request: &ResponsesRequest) -> RoutingStrategy {
    match request
        .extra
        .get("strategy")
        .and_then(|value| value.as_str())
    {
        Some("save") => RoutingStrategy::Save,
        Some("fast") => RoutingStrategy::Fast,
        _ => RoutingStrategy::Auto,
    }
}

async fn provider_responses(
    state: Arc<AppState>,
    request: ResponsesRequest,
    mut model_request: ModelRequest,
) -> axum::response::Response {
    model_request.model = "auto".into();
    let capabilities = state.registry.capabilities().to_vec();
    let provider_health = state
        .telemetry
        .get_provider_health()
        .await
        .unwrap_or_default();
    let Some(decision) = state
        .engine
        .route_with_health(
            &model_request,
            &capabilities,
            provider_strategy(&request),
            &provider_health,
        )
        .await
    else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "No provider supports this Responses request",
                    "type": "no_provider_available"
                }
            })),
        )
            .into_response();
    };

    tracing::info!(
        "Provider Responses route: {} -> {} via {} tier={:?} (stream={})",
        request.model,
        decision.model_id,
        decision.provider_id,
        decision.agent_tier,
        request.stream,
    );

    if request.stream {
        provider_stream_responses(state, model_request, decision).await
    } else {
        provider_non_stream_responses(state, model_request, decision).await
    }
}

fn provider_attempts(state: &AppState, decision: &RoutingDecision) -> Vec<RoutingDecision> {
    let fallback_scores = decision
        .candidates
        .iter()
        .filter(|score| score.provider_id != decision.provider_id)
        .cloned()
        .collect();
    let mut attempts = vec![decision.clone()];
    attempts.extend(
        state
            .engine
            .selector
            .get_fallback_candidates(decision, fallback_scores),
    );
    attempts
}

async fn provider_non_stream_responses(
    state: Arc<AppState>,
    request: ModelRequest,
    decision: RoutingDecision,
) -> axum::response::Response {
    let mut fallback_chain = Vec::new();
    for mut attempt in provider_attempts(&state, &decision) {
        attempt.fallback_chain = fallback_chain.clone();
        let Some(provider) = state.registry.get(&attempt.provider_id).cloned() else {
            fallback_chain.push(provider_route_attempt(
                &attempt,
                RouteAttemptStatus::Failed,
                Some("provider not found".into()),
            ));
            continue;
        };

        match chat_completion_with_timeout(&provider, &request, &attempt.model_id).await {
            Ok(response) => {
                attempt.fallback_chain.push(provider_route_attempt(
                    &attempt,
                    RouteAttemptStatus::Success,
                    None,
                ));
                state
                    .engine
                    .circuit_breaker
                    .record_success(&attempt.provider_id)
                    .await;
                record_provider_telemetry(&state, &attempt, &response, true, None).await;
                return provider_json_response(
                    StatusCode::OK,
                    chat_completion_to_response(&response),
                    &attempt,
                );
            }
            Err(error) => {
                state
                    .engine
                    .circuit_breaker
                    .record_failure(&attempt.provider_id)
                    .await;
                record_provider_telemetry(
                    &state,
                    &attempt,
                    &failed_provider_response(&attempt),
                    false,
                    Some(error.to_string()),
                )
                .await;
                fallback_chain.push(provider_route_attempt(
                    &attempt,
                    RouteAttemptStatus::Failed,
                    Some(error.to_string()),
                ));
            }
        }
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "error": {
                "message": "All compatible providers failed",
                "type": "provider_error"
            },
            "routing": {"fallback_chain": fallback_chain}
        })),
    )
        .into_response()
}

async fn provider_stream_responses(
    state: Arc<AppState>,
    request: ModelRequest,
    decision: RoutingDecision,
) -> axum::response::Response {
    let mut fallback_chain = Vec::new();
    for mut attempt in provider_attempts(&state, &decision) {
        attempt.fallback_chain = fallback_chain.clone();
        let Some(provider) = state.registry.get(&attempt.provider_id).cloned() else {
            fallback_chain.push(provider_route_attempt(
                &attempt,
                RouteAttemptStatus::Failed,
                Some("provider not found".into()),
            ));
            continue;
        };

        match chat_completion_stream_with_timeout(&provider, &request, &attempt.model_id).await {
            Ok(stream) => {
                attempt.fallback_chain.push(provider_route_attempt(
                    &attempt,
                    RouteAttemptStatus::Success,
                    None,
                ));
                return provider_sse_response(state, stream, attempt);
            }
            Err(error) => {
                state
                    .engine
                    .circuit_breaker
                    .record_failure(&attempt.provider_id)
                    .await;
                record_provider_telemetry(
                    &state,
                    &attempt,
                    &failed_provider_response(&attempt),
                    false,
                    Some(error.to_string()),
                )
                .await;
                fallback_chain.push(provider_route_attempt(
                    &attempt,
                    RouteAttemptStatus::Failed,
                    Some(error.to_string()),
                ));
            }
        }
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "error": {
                "message": "All compatible providers failed before streaming",
                "type": "provider_error"
            },
            "routing": {"fallback_chain": fallback_chain}
        })),
    )
        .into_response()
}

struct ProviderSseRuntime {
    upstream: Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    converter: ResponsesStreamState,
    pending: VecDeque<ResponsesSseEvent>,
    usage: Option<Usage>,
    state: Arc<AppState>,
    decision: RoutingDecision,
    started_at: Instant,
    finished: bool,
}

fn provider_sse_response(
    state: Arc<AppState>,
    upstream: Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    decision: RoutingDecision,
) -> axum::response::Response {
    let mut converter = ResponsesStreamState::new(decision.model_id.clone());
    let pending = converter.start().into();
    let runtime = ProviderSseRuntime {
        upstream,
        converter,
        pending,
        usage: None,
        state,
        decision: decision.clone(),
        started_at: Instant::now(),
        finished: false,
    };

    let stream = futures::stream::unfold(runtime, |mut runtime| async move {
        loop {
            if let Some(event) = runtime.pending.pop_front() {
                let event_type = event.event_type.clone();
                return Some((
                    Ok::<Event, Infallible>(
                        Event::default()
                            .event(event_type)
                            .data(event.data.to_string()),
                    ),
                    runtime,
                ));
            }
            if runtime.finished {
                return None;
            }

            match runtime.upstream.next().await {
                Some(Ok(chunk)) => {
                    if let Some(usage) = chunk.usage.clone() {
                        runtime.usage = Some(usage);
                    }
                    runtime.pending.extend(runtime.converter.push(&chunk));
                }
                Some(Err(error)) => {
                    runtime
                        .state
                        .engine
                        .circuit_breaker
                        .record_failure(&runtime.decision.provider_id)
                        .await;
                    record_provider_telemetry(
                        &runtime.state,
                        &runtime.decision,
                        &failed_provider_response(&runtime.decision),
                        false,
                        Some(error.to_string()),
                    )
                    .await;
                    runtime
                        .pending
                        .push_back(runtime.converter.fail(&error.to_string()));
                    runtime.finished = true;
                }
                None => {
                    let usage = runtime.usage.clone().unwrap_or(Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    });
                    let response = ChatCompletionResponse {
                        id: String::new(),
                        model: runtime.decision.model_id.clone(),
                        provider: runtime.decision.provider_id.clone(),
                        choices: Vec::new(),
                        usage: usage.clone(),
                        finish_reason: Some("stop".into()),
                        latency_ms: runtime.started_at.elapsed().as_millis() as u64,
                    };
                    runtime
                        .state
                        .engine
                        .circuit_breaker
                        .record_success(&runtime.decision.provider_id)
                        .await;
                    record_provider_telemetry(
                        &runtime.state,
                        &runtime.decision,
                        &response,
                        true,
                        None,
                    )
                    .await;
                    runtime
                        .pending
                        .extend(runtime.converter.finish(Some(usage)));
                    runtime.finished = true;
                }
            }
        }
    });

    let mut response = Sse::new(stream).into_response();
    add_provider_headers(response.headers_mut(), &decision);
    response
}

fn provider_json_response(
    status: StatusCode,
    value: serde_json::Value,
    decision: &RoutingDecision,
) -> axum::response::Response {
    let mut response = (status, Json(value)).into_response();
    add_provider_headers(response.headers_mut(), decision);
    response
}

fn add_provider_headers(headers: &mut HeaderMap, decision: &RoutingDecision) {
    if let Ok(value) = decision.provider_id.parse() {
        headers.insert("x-dispatcher-provider", value);
    }
    if let Ok(value) = decision.model_id.parse() {
        headers.insert("x-dispatcher-model", value);
    }
    if let Ok(value) = format!("{:?}", decision.agent_tier).to_lowercase().parse() {
        headers.insert("x-dispatcher-agent-tier", value);
    }
}

fn provider_route_attempt(
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

fn failed_provider_response(decision: &RoutingDecision) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: String::new(),
        model: decision.model_id.clone(),
        provider: decision.provider_id.clone(),
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

async fn record_provider_telemetry(
    state: &Arc<AppState>,
    decision: &RoutingDecision,
    response: &ChatCompletionResponse,
    success: bool,
    error_message: Option<String>,
) {
    let cost_usd = decision
        .candidates
        .iter()
        .find(|candidate| {
            candidate.provider_id == decision.provider_id && candidate.model_id == decision.model_id
        })
        .map(|candidate| {
            (response.usage.prompt_tokens as f64 / 1000.0) * candidate.input_cost_per_1k
                + (response.usage.completion_tokens as f64 / 1000.0) * candidate.output_cost_per_1k
        })
        .unwrap_or(0.0);
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
            cost_usd,
            success,
            error_message,
            routing_strategy: format!("{:?}", decision.strategy),
            agent_tier: format!("{:?}", decision.agent_tier).to_lowercase(),
            is_fallback: decision.is_fallback,
        })
        .await;
}

async fn forward_codex_response(
    state: &Arc<AppState>,
    requested_model: &str,
    upstream_body: serde_json::Value,
    route: &CodexRoute,
    stream: bool,
    auth: Option<CodexAuth>,
) -> axum::response::Response {
    let started_at = Instant::now();
    let Some(auth) = auth else {
        record_codex_outcome(
            state,
            requested_model,
            route,
            false,
            Some(StatusCode::SERVICE_UNAVAILABLE),
            started_at.elapsed().as_millis() as u64,
            Some("Codex credentials are not configured".into()),
        )
        .await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Codex native routing requires a server API key or forwarded Codex OpenAI authentication",
                    "type": "codex_credentials_missing"
                },
                "routing": {
                    "model": route.model,
                    "reasoning_effort": route.reasoning_effort,
                    "speed": codex_speed_label(route.speed),
                    "agent_tier": route.agent_tier,
                    "reason": route.reason,
                }
            })),
        )
            .into_response();
    };
    let upstream_body = prepare_codex_upstream_body_for_auth(upstream_body, route, &auth);

    let base_url = std::env::var("DISPATCHER_CODEX_BASE_URL").ok();
    let url = codex_upstream_url_for_auth(base_url.as_deref(), &auth);
    let client = match build_client(std::time::Duration::from_secs(300)) {
        Ok(client) => client,
        Err(error) => {
            record_codex_outcome(
                state,
                requested_model,
                route,
                false,
                Some(StatusCode::INTERNAL_SERVER_ERROR),
                started_at.elapsed().as_millis() as u64,
                Some(error.to_string()),
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": {"message": error.to_string(), "type": "client_initialization_error"}
                })),
            )
                .into_response();
        }
    };

    let mut effective_route = route.clone();
    let mut response = match send_codex_upstream(&client, &url, &auth, &upstream_body, stream).await
    {
        Ok(response) => response,
        Err(CodexUpstreamSendError::Request(error)) => {
            record_codex_outcome(
                state,
                requested_model,
                route,
                false,
                Some(StatusCode::BAD_GATEWAY),
                started_at.elapsed().as_millis() as u64,
                Some(error.to_string()),
            )
            .await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {"message": error.to_string(), "type": "codex_upstream_error"}
                })),
            )
                .into_response();
        }
        Err(CodexUpstreamSendError::Timeout) => {
            let message = format!(
                "Codex upstream connection exceeded {} seconds",
                provider_attempt_timeout().as_secs()
            );
            record_codex_outcome(
                state,
                requested_model,
                route,
                false,
                Some(StatusCode::GATEWAY_TIMEOUT),
                started_at.elapsed().as_millis() as u64,
                Some(message.clone()),
            )
            .await;
            return (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({
                    "error": {
                        "message": message,
                        "type": "codex_upstream_timeout"
                    }
                })),
            )
                .into_response();
        }
    };

    if should_retry_chatgpt_without_fast(response.status(), &effective_route, &auth) {
        let retry_body = without_service_tier(upstream_body);
        effective_route.speed = CodexSpeed::Standard;
        effective_route.reason = format!(
            "{}; fast unavailable, retried with standard speed",
            route.reason
        );
        response = match send_codex_upstream(&client, &url, &auth, &retry_body, stream).await {
            Ok(response) => response,
            Err(CodexUpstreamSendError::Request(error)) => {
                record_codex_outcome(
                    state,
                    requested_model,
                    &effective_route,
                    false,
                    Some(StatusCode::BAD_GATEWAY),
                    started_at.elapsed().as_millis() as u64,
                    Some(error.to_string()),
                )
                .await;
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": {"message": error.to_string(), "type": "codex_upstream_error"}
                    })),
                )
                    .into_response();
            }
            Err(CodexUpstreamSendError::Timeout) => {
                let message = format!(
                    "Codex upstream connection exceeded {} seconds",
                    provider_attempt_timeout().as_secs()
                );
                record_codex_outcome(
                    state,
                    requested_model,
                    &effective_route,
                    false,
                    Some(StatusCode::GATEWAY_TIMEOUT),
                    started_at.elapsed().as_millis() as u64,
                    Some(message.clone()),
                )
                .await;
                return (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(serde_json::json!({
                        "error": {
                            "message": message,
                            "type": "codex_upstream_timeout"
                        }
                    })),
                )
                    .into_response();
            }
        };
    }

    let status = response.status();
    let telemetry_error =
        (!status.is_success()).then(|| format!("Codex upstream returned HTTP {status}"));
    record_codex_outcome(
        state,
        requested_model,
        &effective_route,
        status.is_success(),
        Some(status),
        started_at.elapsed().as_millis() as u64,
        telemetry_error,
    )
    .await;
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let upstream_request_id = response.headers().get("x-request-id").cloned();
    let mut builder = axum::response::Response::builder()
        .status(status)
        .header("x-dispatcher-codex-model", effective_route.model.as_str())
        .header(
            "x-dispatcher-reasoning-effort",
            effective_route.reasoning_effort.as_str(),
        )
        .header(
            "x-dispatcher-speed",
            codex_speed_label(effective_route.speed),
        )
        .header(
            "x-dispatcher-speed-fallback",
            if effective_route.speed != route.speed {
                "true"
            } else {
                "false"
            },
        );
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if let Some(request_id) = upstream_request_id {
        builder = builder.header("x-request-id", request_id);
    }

    builder
        .body(Body::from_stream(response.bytes_stream()))
        .unwrap_or_else(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": {"message": error.to_string(), "type": "response_build_error"}
                })),
            )
                .into_response()
        })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/responses", post(responses))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatcher_engine::{RoutingConfig, RoutingEngine};
    use dispatcher_providers::{demo::DemoProvider, ProviderRegistry};

    struct ToolStreamProvider {
        capability: ProviderCapability,
    }

    impl ToolStreamProvider {
        fn new() -> Self {
            Self {
                capability: ProviderCapability {
                    provider_id: "tool-test".into(),
                    provider_name: "Tool Test".into(),
                    supported_models: vec![ModelInfo {
                        model_id: "tool-test-model".into(),
                        display_name: "Tool Test Model".into(),
                        input_cost_per_1k: 0.0,
                        output_cost_per_1k: 0.0,
                        pricing_source: None,
                        pricing_updated_at: None,
                        supports_streaming: Some(true),
                        supports_tools: Some(true),
                        supports_vision: Some(false),
                        max_tokens: 8192,
                        quality_score: 0.8,
                        avg_latency_ms: 1,
                    }],
                    base_url: "local://tool-test".into(),
                    requires_api_key: false,
                    supports_streaming: true,
                    supports_tools: true,
                    supports_vision: false,
                    max_context_length: 8192,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for ToolStreamProvider {
        fn provider_id(&self) -> &str {
            "tool-test"
        }

        fn capability(&self) -> &ProviderCapability {
            &self.capability
        }

        async fn health_check(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        async fn chat_completion(
            &self,
            _request: &ModelRequest,
            _model_id: &str,
        ) -> Result<ChatCompletionResponse, ProviderError> {
            unreachable!("stream test provider only")
        }

        async fn chat_completion_stream(
            &self,
            _request: &ModelRequest,
            _model_id: &str,
        ) -> Result<
            Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
            ProviderError,
        > {
            let chunks = vec![
                Ok(StreamChunk {
                    id: "chunk_1".into(),
                    model: "tool-test-model".into(),
                    choices: vec![StreamChoice {
                        index: 0,
                        delta: StreamDelta {
                            role: Some("assistant".into()),
                            content: None,
                            reasoning_content: None,
                            tool_calls: Some(vec![ToolCall {
                                index: Some(0),
                                id: "call_1".into(),
                                call_type: "function".into(),
                                function: FunctionCall {
                                    name: "exec_command".into(),
                                    arguments: "{\"cmd\":\"".into(),
                                },
                            }]),
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                }),
                Ok(StreamChunk {
                    id: "chunk_1".into(),
                    model: "tool-test-model".into(),
                    choices: vec![StreamChoice {
                        index: 0,
                        delta: StreamDelta {
                            role: None,
                            content: None,
                            reasoning_content: None,
                            tool_calls: Some(vec![ToolCall {
                                index: Some(0),
                                id: String::new(),
                                call_type: "function".into(),
                                function: FunctionCall {
                                    name: String::new(),
                                    arguments: "pwd\"}".into(),
                                },
                            }]),
                        },
                        finish_reason: Some("tool_calls".into()),
                    }],
                    usage: Some(Usage {
                        prompt_tokens: 10,
                        completion_tokens: 4,
                        total_tokens: 14,
                    }),
                }),
            ];
            Ok(Box::new(futures::stream::iter(chunks)))
        }
    }

    fn request() -> ResponsesRequest {
        serde_json::from_value(serde_json::json!({
            "model": "auto",
            "instructions": "Be concise.",
            "input": [
                {
                    "type": "message",
                    "role": "developer",
                    "content": [{"type":"input_text","text":"Follow project rules."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type":"input_text","text":"Hello"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "tool result"
                }
            ],
            "tools": [{
                "type": "function",
                "name": "exec_command",
                "description": "Run a command",
                "parameters": {"type":"object"}
            }],
            "stream": true
        }))
        .unwrap()
    }

    #[test]
    fn converts_codex_responses_request_to_internal_model_request() {
        let converted = responses_to_model_request(&request());

        assert_eq!(converted.messages.len(), 4);
        assert_eq!(converted.messages[0].role, "system");
        assert_eq!(converted.messages[1].role, "system");
        assert_eq!(converted.messages[2].role, "user");
        assert_eq!(converted.messages[3].role, "user");
        assert_eq!(
            converted.tools.as_ref().unwrap()[0].function.name,
            "exec_command"
        );
        assert!(converted.stream);
    }

    #[test]
    fn accepts_object_arguments_from_codex_tool_search_calls() {
        let request = serde_json::from_value::<ResponsesRequest>(serde_json::json!({
            "model": "dispatcher-auto",
            "input": [{
                "type": "tool_search_call",
                "call_id": "call_search_1",
                "arguments": {
                    "query": "browser automation control local localhost screenshot",
                    "limit": 10
                }
            }],
            "stream": true
        }));

        assert!(request.is_ok(), "{request:?}");
    }

    #[test]
    fn provider_auto_mode_requires_explicit_header_value() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Dispatcher-Mode", "provider-auto".parse().unwrap());
        assert!(provider_auto_header(&headers));

        headers.insert("X-Dispatcher-Mode", "auto".parse().unwrap());
        assert!(!provider_auto_header(&headers));
    }

    #[test]
    fn provider_auto_ignores_client_tools_but_rejects_hosted_tools() {
        let mut request = request();
        assert!(unsupported_provider_tools(&request).is_empty());

        request.tools.push(ResponseTool {
            tool_type: "custom".into(),
            name: "apply_patch".into(),
            description: None,
            parameters: None,
        });
        request.tools.push(ResponseTool {
            tool_type: "tool_search".into(),
            name: String::new(),
            description: None,
            parameters: None,
        });
        assert!(unsupported_provider_tools(&request).is_empty());

        request.tools.push(ResponseTool {
            tool_type: "web_search".into(),
            name: String::new(),
            description: None,
            parameters: None,
        });
        assert_eq!(unsupported_provider_tools(&request), vec!["web_search"]);
    }

    async fn provider_test_state() -> (Arc<AppState>, std::path::PathBuf) {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(DemoProvider::new()));
        let path = std::env::temp_dir().join(format!(
            "dispatcher-provider-responses-{}.db",
            uuid::Uuid::new_v4()
        ));
        let config = RoutingConfig::default();
        let state = Arc::new(AppState {
            engine: RoutingEngine::new(config.clone()),
            registry,
            telemetry: crate::telemetry::TelemetryStore::new(path.to_string_lossy().as_ref())
                .await
                .unwrap(),
            routing_config: config,
            policy_config_path: None,
        });
        (state, path)
    }

    async fn tool_provider_test_state() -> (Arc<AppState>, std::path::PathBuf) {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(ToolStreamProvider::new()));
        let path = std::env::temp_dir().join(format!(
            "dispatcher-tool-provider-responses-{}.db",
            uuid::Uuid::new_v4()
        ));
        let config = RoutingConfig::default();
        let state = Arc::new(AppState {
            engine: RoutingEngine::new(config.clone()),
            registry,
            telemetry: crate::telemetry::TelemetryStore::new(path.to_string_lossy().as_ref())
                .await
                .unwrap(),
            routing_config: config,
            policy_config_path: None,
        });
        (state, path)
    }

    #[tokio::test]
    async fn provider_auto_non_stream_routes_to_demo_as_responses_json() {
        let (state, path) = provider_test_state().await;
        let mut request = route_request("hello provider mode");
        request.stream = false;
        let model_request = responses_to_model_request(&request);

        let response = provider_responses(state.clone(), request, model_request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-dispatcher-provider")
                .unwrap()
                .to_str()
                .unwrap(),
            "demo"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["object"], "response");
        assert_eq!(json["status"], "completed");
        assert_eq!(json["output"][0]["type"], "message");
        assert!(json["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("hello provider mode"));

        drop(state);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn provider_auto_stream_routes_to_demo_as_responses_sse() {
        let (state, path) = provider_test_state().await;
        let request = route_request("hello stream provider mode");
        let model_request = responses_to_model_request(&request);

        let response = provider_responses(state.clone(), request, model_request).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("event: response.created"));
        assert!(body.contains("event: response.output_text.delta"));
        assert!(body.contains("event: response.completed"));
        assert!(body.contains("hello stream provider mode"));

        drop(state);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn provider_auto_stream_preserves_function_call_events() {
        let (state, path) = tool_provider_test_state().await;
        let request = route_request("run pwd");
        let model_request = responses_to_model_request(&request);

        let response = provider_responses(state.clone(), request, model_request).await;
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("event: response.function_call_arguments.delta"));
        assert!(body.contains("event: response.function_call_arguments.done"));
        assert!(body.contains(r#""arguments":"{\"cmd\":\"pwd\"}""#));
        assert!(body.contains(r#""call_id":"call_1""#));
        assert!(body.contains(r#""name":"exec_command""#));

        drop(state);
        std::fs::remove_file(path).unwrap();
    }

    fn route_request(prompt: &str) -> ResponsesRequest {
        serde_json::from_value(serde_json::json!({
            "model": "auto",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type":"input_text","text":prompt}]
            }],
            "tools": [],
            "stream": true
        }))
        .unwrap()
    }

    #[test]
    fn codex_simple_tasks_use_mini_with_low_effort() {
        let request = route_request("继续");
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.4-mini");
        assert_eq!(route.reasoning_effort, "low");
        assert_eq!(route.speed, CodexSpeed::Standard);
    }

    #[test]
    fn codex_medium_tasks_use_5_4_with_medium_effort() {
        let request = route_request("Read package.json and compare it with README.");
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(route.reasoning_effort, "medium");
    }

    #[test]
    fn codex_reasoning_tasks_use_5_5_with_high_effort() {
        let request = route_request(
            "Analyze this multi-file Rust service architecture and debug its error handling.",
        );
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.5");
        assert_eq!(route.reasoning_effort, "high");
    }

    #[test]
    fn codex_complex_tasks_use_5_5_with_xhigh_effort() {
        let request =
            route_request("Orchestrate parallel frontend, backend, security, and test workflows.");
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.5");
        assert_eq!(route.reasoning_effort, "xhigh");
    }

    #[test]
    fn codex_priority_speed_is_used_only_by_models_that_support_it() {
        let mut strong_request = route_request("Implement an async Rust service.");
        strong_request.service_tier = Some("priority".into());
        let strong_route = select_codex_route(
            &strong_request,
            &responses_to_model_request(&strong_request),
        );
        assert_eq!(strong_route.speed, CodexSpeed::Priority);

        let mut simple_request = route_request("继续");
        simple_request.service_tier = Some("priority".into());
        let simple_route = select_codex_route(
            &simple_request,
            &responses_to_model_request(&simple_request),
        );
        assert_eq!(simple_route.model, "gpt-5.4-mini");
        assert_eq!(simple_route.speed, CodexSpeed::Standard);
    }

    #[test]
    fn codex_explicit_native_model_and_effort_are_respected() {
        let mut request = route_request("Analyze this architecture.");
        request.model = "gpt-5.4".into();
        request.reasoning = Some(CodexReasoning {
            effort: Some("xhigh".into()),
            summary: None,
        });

        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(route.reasoning_effort, "xhigh");
    }

    #[test]
    fn dispatcher_auto_ignores_desktop_model_reasoning_and_speed_defaults() {
        let mut request = route_request("Read package.json and compare it with README.");
        request.model = "dispatcher-auto".into();
        request.reasoning = Some(CodexReasoning {
            effort: Some("high".into()),
            summary: Some("auto".into()),
        });
        request.service_tier = Some("auto".into());

        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(route.reasoning_effort, "medium");
        assert_eq!(route.speed, CodexSpeed::Priority);
    }

    #[test]
    fn dispatcher_auto_header_overrides_known_desktop_model_defaults() {
        let mut request = route_request("Read package.json and compare it with README.");
        request.model = "gpt-5.5".into();
        request.reasoning = Some(CodexReasoning {
            effort: Some("high".into()),
            summary: None,
        });

        let route =
            select_codex_route_for_mode(&request, &responses_to_model_request(&request), true);

        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(route.reasoning_effort, "medium");
        assert_eq!(route.speed, CodexSpeed::Priority);
    }

    #[test]
    fn dispatcher_auto_classifies_latest_user_intent_not_desktop_harness_context() {
        let request: ResponsesRequest = serde_json::from_value(serde_json::json!({
            "model": "dispatcher-auto",
            "instructions": "You are a coding agent. Orchestrate parallel workflows, review architecture, debug services, and use tools.",
            "input": [
                {
                    "type": "message",
                    "role": "developer",
                    "content": [{"type":"input_text","text":"Follow the full project test and security policy."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type":"input_text","text":"继续"}]
                }
            ],
            "tools": [{
                "type": "function",
                "name": "exec_command",
                "description": "Run a command",
                "parameters": {"type":"object"}
            }],
            "reasoning": {"effort":"high"},
            "stream": true
        }))
        .unwrap();

        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.agent_tier, AgentTier::Simple);
        assert_eq!(route.model, "gpt-5.4-mini");
        assert_eq!(route.reasoning_effort, "low");
    }

    #[test]
    fn dispatcher_auto_uses_shared_synthetic_context_filter() {
        let request: ResponsesRequest = serde_json::from_value(serde_json::json!({
            "model": "dispatcher-auto",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type":"input_text","text":"Read package.json and compare it with README."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type":"input_text",
                        "text":"<environment_context>parallel agents architecture security audit</environment_context>"
                    }]
                }
            ],
            "tools": [],
            "stream": true
        }))
        .unwrap();

        let route = select_codex_route(&request, &responses_to_model_request(&request));

        assert_eq!(route.agent_tier, AgentTier::Medium);
        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(route.reasoning_effort, "medium");
        assert_eq!(route.speed, CodexSpeed::Priority);
    }

    #[test]
    fn dispatcher_auto_route_stays_inside_subscription_matrix() {
        for prompt in [
            "继续",
            "Read package.json and compare it with README.",
            "Analyze this multi-file Rust service architecture and debug its error handling.",
            "Orchestrate parallel frontend, backend, security, and test workflows.",
        ] {
            let mut request = route_request(prompt);
            request.model = "gpt-5.5".into();
            request.reasoning = Some(CodexReasoning {
                effort: Some("high".into()),
                summary: None,
            });

            let route =
                select_codex_route_for_mode(&request, &responses_to_model_request(&request), true);

            assert!(matches!(
                route.model.as_str(),
                "gpt-5.5" | "gpt-5.4" | "gpt-5.4-mini"
            ));
            assert!(matches!(
                route.reasoning_effort.as_str(),
                "low" | "medium" | "high" | "xhigh"
            ));
            assert!(matches!(
                route.speed,
                CodexSpeed::Standard | CodexSpeed::Priority
            ));
        }
    }

    #[test]
    fn codex_upstream_body_preserves_input_and_applies_route() {
        let request = route_request("Implement an async Rust service.");
        let raw = serde_json::to_value(&request).unwrap();
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        let body = build_codex_upstream_body(raw, &route);

        assert_eq!(body["model"], route.model);
        assert_eq!(body["reasoning"]["effort"], route.reasoning_effort);
        assert_eq!(body["service_tier"], "auto");
        assert_eq!(
            body["input"][0]["content"][0]["text"],
            "Implement an async Rust service."
        );
    }

    #[test]
    fn codex_upstream_url_uses_dedicated_base_and_normalizes_slashes() {
        assert_eq!(
            codex_upstream_url(Some("https://example.test/openai/v1/")),
            "https://example.test/openai/v1/responses"
        );
        assert_eq!(
            codex_upstream_url(None),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn codex_api_key_prefers_dedicated_key() {
        assert_eq!(
            codex_api_key_from(Some("codex-key"), Some("openai-key")),
            Some("codex-key".into())
        );
        assert_eq!(
            codex_api_key_from(None, Some("openai-key")),
            Some("openai-key".into())
        );
        assert_eq!(codex_api_key_from(Some(""), Some("")), None);
    }

    #[test]
    fn codex_auth_prefers_chatgpt_subscription_over_server_api_key() {
        let auth = codex_auth_from(
            Some("server-key"),
            Some("openai-key"),
            Some("Bearer desktop-token"),
            Some("account-123"),
        )
        .unwrap();

        assert_eq!(auth.bearer_token, "desktop-token");
        assert_eq!(auth.chatgpt_account_id.as_deref(), Some("account-123"));
    }

    #[test]
    fn codex_auth_accepts_client_chatgpt_login_when_server_key_is_absent() {
        let auth = codex_auth_from(
            None,
            None,
            Some("Bearer desktop-token"),
            Some("account-123"),
        )
        .unwrap();

        assert_eq!(auth.bearer_token, "desktop-token");
        assert_eq!(auth.chatgpt_account_id.as_deref(), Some("account-123"));
        assert_eq!(
            codex_upstream_url_for_auth(None, &auth),
            "https://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn chatgpt_auth_omits_standard_api_service_tier() {
        let request = route_request("继续");
        let route = select_codex_route(&request, &responses_to_model_request(&request));
        let body = build_codex_upstream_body(serde_json::to_value(&request).unwrap(), &route);
        let auth = codex_auth_from(
            None,
            None,
            Some("Bearer desktop-token"),
            Some("account-123"),
        )
        .unwrap();

        let body = prepare_codex_upstream_body_for_auth(body, &route, &auth);

        assert!(body.get("service_tier").is_none());
    }

    #[test]
    fn chatgpt_auth_maps_accelerated_route_to_fast_mode() {
        let mut request = route_request("Read package.json and compare it with README.");
        request.model = "dispatcher-auto".into();
        let route = select_codex_route(&request, &responses_to_model_request(&request));
        assert_eq!(route.speed, CodexSpeed::Priority);
        let body = build_codex_upstream_body(serde_json::to_value(&request).unwrap(), &route);
        let auth = codex_auth_from(
            None,
            None,
            Some("Bearer desktop-token"),
            Some("account-123"),
        )
        .unwrap();

        let body = prepare_codex_upstream_body_for_auth(body, &route, &auth);

        assert_eq!(body["service_tier"], "fast");
    }

    #[test]
    fn detects_chatgpt_fast_mode_rejection_for_standard_retry() {
        let request = route_request("Read package.json and compare it with README.");
        let mut route = select_codex_route(&request, &responses_to_model_request(&request));
        route.speed = CodexSpeed::Priority;
        let auth = codex_auth_from(
            None,
            None,
            Some("Bearer desktop-token"),
            Some("account-123"),
        )
        .unwrap();

        assert!(should_retry_chatgpt_without_fast(
            StatusCode::BAD_REQUEST,
            &route,
            &auth
        ));
        assert!(!should_retry_chatgpt_without_fast(
            StatusCode::UNAUTHORIZED,
            &route,
            &auth
        ));
    }

    #[test]
    fn codex_auth_rejects_placeholder_or_malformed_client_tokens() {
        assert!(codex_auth_from(None, None, Some("local-dispatcher"), None).is_none());
        assert!(codex_auth_from(None, None, Some("Bearer "), None).is_none());
    }

    #[test]
    fn codex_telemetry_record_preserves_the_route_decision() {
        let mut request = route_request("Analyze this architecture and debug the service.");
        request.service_tier = Some("priority".into());
        let route = select_codex_route(&request, &responses_to_model_request(&request));

        let record = codex_telemetry_record(
            request.model.as_str(),
            &route,
            true,
            Some(StatusCode::OK),
            245,
            None,
        );

        assert_eq!(record.requested_model, "auto");
        assert_eq!(record.model_id, "gpt-5.5");
        assert_eq!(record.reasoning_effort, "high");
        assert_eq!(record.speed, "priority");
        assert_eq!(record.agent_tier, "reasoning");
        assert_eq!(record.status_code, Some(200));
        assert_eq!(record.latency_ms, 245);
        assert!(record.success);
    }
}
