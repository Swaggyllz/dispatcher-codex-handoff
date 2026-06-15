pub mod analyzer;
pub mod circuit_breaker;
pub mod scorer;
pub mod selector;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use analyzer::RequestAnalyzer;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerSnapshot, CircuitBreakerState};
pub use scorer::ProviderScorer;
pub use selector::RouteSelector;
pub use types::*;

/// 路由引擎 — 组合 Analyzer → Scorer → Selector pipeline
pub struct RoutingEngine {
    pub analyzer: RequestAnalyzer,
    pub scorer: ProviderScorer,
    pub selector: RouteSelector,
    pub circuit_breaker: CircuitBreaker,
    sticky_sessions: Arc<RwLock<HashMap<String, StickyRoute>>>,
}

#[derive(Debug, Clone)]
struct StickyRoute {
    provider_id: String,
    model_id: String,
    agent_tier: AgentTier,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn model_request(content: MessageContent) -> ModelRequest {
        ModelRequest {
            model: "auto".into(),
            messages: vec![Message {
                role: "user".into(),
                content,
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: None,
            extra: Default::default(),
        }
    }

    fn capability(
        provider_id: &str,
        model_id: &str,
        quality_score: f64,
        supports_tools: bool,
        max_context_length: usize,
    ) -> ProviderCapability {
        capability_with_cost(
            provider_id,
            model_id,
            quality_score,
            0.001,
            supports_tools,
            max_context_length,
        )
    }

    fn capability_with_cost(
        provider_id: &str,
        model_id: &str,
        quality_score: f64,
        input_cost_per_1k: f64,
        supports_tools: bool,
        max_context_length: usize,
    ) -> ProviderCapability {
        ProviderCapability {
            provider_id: provider_id.into(),
            provider_name: provider_id.into(),
            supported_models: vec![ModelInfo {
                model_id: model_id.into(),
                display_name: model_id.into(),
                input_cost_per_1k,
                output_cost_per_1k: input_cost_per_1k * 2.0,
                pricing_source: None,
                pricing_updated_at: None,
                supports_streaming: None,
                supports_tools: None,
                supports_vision: None,
                max_tokens: max_context_length as u32,
                quality_score,
                avg_latency_ms: 1000,
            }],
            base_url: String::new(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools,
            supports_vision: true,
            max_context_length,
        }
    }

    fn request_with_session(text: &str, session_id: &str) -> ModelRequest {
        let mut request = model_request(MessageContent::Text(text.into()));
        request
            .extra
            .insert("session_id".into(), serde_json::json!(session_id));
        request
    }

    #[tokio::test]
    async fn route_excludes_providers_without_required_tools() {
        let mut request = model_request(MessageContent::Text(
            "Use a tool to inspect this repo".into(),
        ));
        request.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "read_file".into(),
                description: None,
                parameters: None,
            },
        }]);

        let engine = RoutingEngine::new(RoutingConfig::default());
        let decision = engine
            .route(
                &request,
                &[
                    capability("no-tools", "strong-model", 0.99, false, 128_000),
                    capability("with-tools", "capable-model", 0.70, true, 128_000),
                ],
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();

        assert_eq!(decision.provider_id, "with-tools");
    }

    #[tokio::test]
    async fn route_records_exclusion_reason_for_unsupported_tools() {
        let mut request = model_request(MessageContent::Text(
            "Use a tool to inspect this repo".into(),
        ));
        request.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "read_file".into(),
                description: None,
                parameters: None,
            },
        }]);

        let engine = RoutingEngine::new(RoutingConfig::default());
        let decision = engine
            .route(
                &request,
                &[
                    capability("no-tools", "strong-model", 0.99, false, 128_000),
                    capability("with-tools", "capable-model", 0.70, true, 128_000),
                ],
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();

        assert_eq!(decision.excluded_candidates.len(), 1);
        assert_eq!(decision.excluded_candidates[0].provider_id, "no-tools");
        assert_eq!(decision.excluded_candidates[0].reason, "tools unsupported");
    }

    #[tokio::test]
    async fn route_excludes_models_with_model_level_unsupported_tools() {
        let mut request = model_request(MessageContent::Text(
            "Use a tool to inspect this repo".into(),
        ));
        request.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "read_file".into(),
                description: None,
                parameters: None,
            },
        }]);

        let mut capability = capability("mixed", "cheap-no-tools", 0.99, true, 128_000);
        capability.supported_models[0].supports_tools = Some(false);
        capability.supported_models.push(ModelInfo {
            model_id: "tool-capable".into(),
            display_name: "Tool Capable".into(),
            input_cost_per_1k: 0.01,
            output_cost_per_1k: 0.02,
            pricing_source: None,
            pricing_updated_at: None,
            supports_streaming: None,
            supports_tools: Some(true),
            supports_vision: None,
            max_tokens: 128_000,
            quality_score: 0.7,
            avg_latency_ms: 1000,
        });

        let engine = RoutingEngine::new(RoutingConfig::default());
        let decision = engine
            .route(&request, &[capability], RoutingStrategy::Auto)
            .await
            .unwrap();

        assert_eq!(decision.model_id, "tool-capable");
        assert!(decision.excluded_candidates.iter().any(|candidate| {
            candidate.provider_id == "mixed"
                && candidate.model_id.as_deref() == Some("cheap-no-tools")
                && candidate.reason == "tools unsupported"
        }));
    }

    #[tokio::test]
    async fn route_excludes_providers_with_insufficient_context() {
        let request = model_request(MessageContent::Text("a".repeat(100_000)));

        let engine = RoutingEngine::new(RoutingConfig::default());
        let decision = engine
            .route(
                &request,
                &[
                    capability("short-context", "strong-model", 0.99, true, 8_192),
                    capability("long-context", "long-model", 0.70, true, 128_000),
                ],
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();

        assert_eq!(decision.provider_id, "long-context");
    }

    #[tokio::test]
    async fn short_continuation_reuses_previous_session_route() {
        let engine = RoutingEngine::new(RoutingConfig::default());
        let capabilities = [
            capability_with_cost("premium", "claude-opus-4-7", 0.98, 0.015, true, 128_000),
            capability_with_cost("budget", "qwen-flash", 0.65, 0.0001, true, 128_000),
        ];

        let first = engine
            .route(
                &request_with_session("请实现一个复杂的 Rust API，并分析错误处理", "s1"),
                &capabilities,
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();
        assert_eq!(first.provider_id, "premium");
        assert_eq!(first.agent_tier, AgentTier::Reasoning);

        let continuation = engine
            .route(
                &request_with_session("继续", "s1"),
                &capabilities,
                RoutingStrategy::Save,
            )
            .await
            .unwrap();

        assert_eq!(continuation.provider_id, "premium");
        assert_eq!(continuation.model_id, "claude-opus-4-7");
    }

    #[tokio::test]
    async fn synthetic_context_after_continuation_keeps_sticky_route() {
        let engine = RoutingEngine::new(RoutingConfig::default());
        let capabilities = [
            capability_with_cost("premium", "claude-opus-4-7", 0.98, 0.015, true, 128_000),
            capability_with_cost("budget", "qwen-flash", 0.65, 0.0001, true, 128_000),
        ];

        let first = engine
            .route(
                &request_with_session("请实现一个复杂的 Rust API，并分析错误处理", "s-context"),
                &capabilities,
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();
        assert_eq!(first.provider_id, "premium");

        let mut continuation = request_with_session("继续", "s-context");
        continuation.messages.push(Message {
            role: "user".into(),
            content: MessageContent::Text(
                "<environment_context>parallel agents architecture audit</environment_context>"
                    .into(),
            ),
        });
        let continued = engine
            .route(&continuation, &capabilities, RoutingStrategy::Save)
            .await
            .unwrap();

        assert_eq!(continued.provider_id, "premium");
        assert_eq!(
            continued.fallback_reason.as_deref(),
            Some("sticky_session_continuation")
        );
    }

    #[tokio::test]
    async fn non_continuation_in_same_session_is_rerouted() {
        let engine = RoutingEngine::new(RoutingConfig::default());
        let capabilities = [
            capability_with_cost("premium", "claude-opus-4-7", 0.98, 0.015, true, 128_000),
            capability_with_cost("budget", "qwen-flash", 0.65, 0.0001, true, 128_000),
        ];

        let first = engine
            .route(
                &request_with_session("请实现一个复杂的 Rust API，并分析错误处理", "s2"),
                &capabilities,
                RoutingStrategy::Auto,
            )
            .await
            .unwrap();
        assert_eq!(first.provider_id, "premium");

        let new_question = engine
            .route(
                &request_with_session("今天天气怎么样？", "s2"),
                &capabilities,
                RoutingStrategy::Save,
            )
            .await
            .unwrap();

        assert_eq!(new_question.provider_id, "budget");
        assert_eq!(new_question.agent_tier, AgentTier::Simple);
    }

    #[tokio::test]
    async fn route_with_health_avoids_provider_with_poor_recent_success_rate() {
        let engine = RoutingEngine::new(RoutingConfig::default());
        let capabilities = [
            capability_with_cost("healthy", "model-a", 0.82, 0.001, true, 128_000),
            capability_with_cost("unreliable", "model-b", 0.82, 0.001, true, 128_000),
        ];
        let health = HashMap::from([
            (
                "healthy".to_string(),
                ProviderHealthSnapshot {
                    provider_id: "healthy".into(),
                    sample_count: 10,
                    success_rate: 1.0,
                    avg_latency_ms: 900,
                },
            ),
            (
                "unreliable".to_string(),
                ProviderHealthSnapshot {
                    provider_id: "unreliable".into(),
                    sample_count: 10,
                    success_rate: 0.3,
                    avg_latency_ms: 900,
                },
            ),
        ]);

        let decision = engine
            .route_with_health(
                &model_request(MessageContent::Text(
                    "Implement a small parser function".into(),
                )),
                &capabilities,
                RoutingStrategy::Auto,
                &health,
            )
            .await
            .unwrap();

        assert_eq!(decision.provider_id, "healthy");
    }
}

impl RoutingEngine {
    pub fn new(config: RoutingConfig) -> Self {
        let scorer_config = config.clone();
        let selector_config = config.clone();
        let cb_threshold = config.circuit_breaker_threshold;
        let cb_timeout = config.circuit_breaker_timeout_secs;

        Self {
            analyzer: RequestAnalyzer,
            scorer: ProviderScorer::new(scorer_config),
            selector: RouteSelector::new(selector_config),
            circuit_breaker: CircuitBreaker::new(cb_threshold, cb_timeout),
            sticky_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 完整的路由决策流程
    pub async fn route(
        &self,
        request: &ModelRequest,
        capabilities: &[ProviderCapability],
        strategy: RoutingStrategy,
    ) -> Option<RoutingDecision> {
        self.route_with_health(request, capabilities, strategy, &HashMap::new())
            .await
    }

    pub async fn route_with_health(
        &self,
        request: &ModelRequest,
        capabilities: &[ProviderCapability],
        strategy: RoutingStrategy,
        health: &HashMap<String, ProviderHealthSnapshot>,
    ) -> Option<RoutingDecision> {
        // 1. 分析请求特征
        let features = RequestAnalyzer::analyze(request);
        tracing::debug!(
            "Request analyzed: complexity={:.2}, task={:?}, tokens={}",
            features.complexity_score,
            features.task_type,
            features.estimated_tokens
        );

        // 2. 过滤被熔断的 provider
        let open_providers = self.circuit_breaker.get_open_providers().await;
        let mut excluded_candidates = Vec::new();
        let mut available_caps = Vec::new();
        for capability in capabilities {
            if open_providers.contains(&capability.provider_id) {
                excluded_candidates.push(ExcludedCandidate {
                    provider_id: capability.provider_id.clone(),
                    model_id: None,
                    reason: "provider circuit breaker open".into(),
                });
                continue;
            }
            if let Some(reason) = Self::request_rejection_reason(capability, &features) {
                excluded_candidates.push(ExcludedCandidate {
                    provider_id: capability.provider_id.clone(),
                    model_id: None,
                    reason,
                });
                continue;
            }
            let mut available_capability = capability.clone();
            available_capability.supported_models.retain(|model| {
                if let Some(reason) = Self::model_rejection_reason(capability, model, &features) {
                    excluded_candidates.push(ExcludedCandidate {
                        provider_id: capability.provider_id.clone(),
                        model_id: Some(model.model_id.clone()),
                        reason,
                    });
                    false
                } else {
                    true
                }
            });
            if !available_capability.supported_models.is_empty() {
                available_caps.push(available_capability);
            }
        }

        // 3. 打分
        let mut scores =
            self.scorer
                .score_all_with_health(&available_caps, &features, strategy, health);

        // 4. 应用熔断器状态到评分
        for score in &mut scores {
            if open_providers.contains(&score.provider_id) {
                score.availability_score = 0.0;
                score.availability = AvailabilityStatus::Unavailable;
            }
        }

        // 5. 排除被熔断的 provider
        let excluded = open_providers;

        // 6. 短确认延续上一轮模型，保护 coding-agent 连续动作的上下文稳定性
        if let Some(decision) = self
            .sticky_decision(request, &scores, strategy, excluded_candidates.clone())
            .await
        {
            return Some(decision);
        }

        // 7. 选择
        let mut decision =
            self.selector
                .select(scores.clone(), strategy, features.agent_tier, &excluded)?;
        decision.excluded_candidates = excluded_candidates;

        if let Some(session_id) = Self::session_id(request) {
            let mut sticky = self.sticky_sessions.write().await;
            sticky.insert(
                session_id.to_string(),
                StickyRoute {
                    provider_id: decision.provider_id.clone(),
                    model_id: decision.model_id.clone(),
                    agent_tier: decision.agent_tier,
                },
            );
        }

        Some(decision)
    }

    fn request_rejection_reason(
        capability: &ProviderCapability,
        features: &RequestFeatures,
    ) -> Option<String> {
        if features.is_streaming && !capability.supports_streaming {
            return Some("streaming unsupported".into());
        }
        if features.has_tools && !capability.supports_tools {
            return Some("tools unsupported".into());
        }
        if features.has_images && !capability.supports_vision {
            return Some("vision unsupported".into());
        }
        if features.estimated_tokens > capability.max_context_length {
            return Some(format!(
                "context too short: estimated {} tokens exceeds provider limit {}",
                features.estimated_tokens, capability.max_context_length
            ));
        }
        None
    }

    fn model_rejection_reason(
        capability: &ProviderCapability,
        model: &ModelInfo,
        features: &RequestFeatures,
    ) -> Option<String> {
        if features.is_streaming
            && capability.supports_streaming
            && model.supports_streaming == Some(false)
        {
            return Some("streaming unsupported".into());
        }
        if features.has_tools && capability.supports_tools && model.supports_tools == Some(false) {
            return Some("tools unsupported".into());
        }
        if features.has_images && capability.supports_vision && model.supports_vision == Some(false)
        {
            return Some("vision unsupported".into());
        }
        None
    }

    async fn sticky_decision(
        &self,
        request: &ModelRequest,
        scores: &[ProviderScore],
        strategy: RoutingStrategy,
        excluded_candidates: Vec<ExcludedCandidate>,
    ) -> Option<RoutingDecision> {
        let session_id = Self::session_id(request)?;
        if !Self::is_short_continuation(request) {
            return None;
        }

        let sticky = self.sticky_sessions.read().await;
        let previous = sticky.get(session_id)?;
        let previous_score = scores
            .iter()
            .find(|score| {
                score.provider_id == previous.provider_id && score.model_id == previous.model_id
            })?
            .clone();

        let mut candidates = scores.to_vec();
        candidates.sort_by(|a, b| {
            if a.provider_id == previous_score.provider_id && a.model_id == previous_score.model_id
            {
                std::cmp::Ordering::Less
            } else if b.provider_id == previous_score.provider_id
                && b.model_id == previous_score.model_id
            {
                std::cmp::Ordering::Greater
            } else {
                b.total_score.partial_cmp(&a.total_score).unwrap()
            }
        });

        Some(RoutingDecision {
            provider_id: previous.provider_id.clone(),
            model_id: previous.model_id.clone(),
            strategy,
            agent_tier: previous.agent_tier,
            candidates,
            decision_time_ms: 0,
            is_fallback: false,
            fallback_reason: Some("sticky_session_continuation".into()),
            fallback_chain: Vec::new(),
            policy_reason: previous_score.policy_reason,
            decision_reason: format!(
                "Reused sticky session route {}/{} for short continuation",
                previous.provider_id, previous.model_id
            ),
            excluded_candidates,
        })
    }

    fn session_id(request: &ModelRequest) -> Option<&str> {
        ["session_id", "sessionId", "conversation_id", "thread_id"]
            .iter()
            .find_map(|key| request.extra.get(*key).and_then(|value| value.as_str()))
    }

    fn is_short_continuation(request: &ModelRequest) -> bool {
        let Some(text) = RequestAnalyzer::latest_user_intent_text(request) else {
            return false;
        };

        let normalized = text.trim().to_lowercase();
        if normalized.chars().count() > 30 {
            return false;
        }

        matches!(
            normalized.as_str(),
            "ok" | "okay"
                | "yes"
                | "y"
                | "sure"
                | "go"
                | "go ahead"
                | "do it"
                | "continue"
                | "next"
                | "proceed"
                | "run"
                | "好的"
                | "好"
                | "继续"
                | "开始"
                | "可以"
                | "行"
                | "嗯"
                | "对"
                | "是的"
                | "没问题"
                | "执行"
                | "开搞"
                | "冲"
        )
    }
}
