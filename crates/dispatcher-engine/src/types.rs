use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub use crate::handoff_certification::{HandoffCertification, HandoffCertificationLabel};

fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// 模型请求，OpenAI-compatible 格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub tools: Option<Vec<Tool>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
    pub image_url: Option<ImageUrl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

/// 请求特征（由 Analyzer 提取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFeatures {
    /// 估算的 token 数
    pub estimated_tokens: usize,
    /// 是否包含工具调用
    pub has_tools: bool,
    /// 是否包含图片
    pub has_images: bool,
    /// 是否是流式请求
    pub is_streaming: bool,
    /// 任务复杂度评分 0.0-1.0
    pub complexity_score: f64,
    /// 任务类型
    pub task_type: TaskType,
    /// 面向 AI 编程代理的任务层级
    pub agent_tier: AgentTier,
    /// 是否为长上下文（>32K tokens）
    pub is_long_context: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Chat,
    Code,
    Analysis,
    Creative,
    Translation,
    Summarization,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTier {
    /// 问候、确认、短问答
    Simple,
    /// 单步代码/文本任务，短上下文
    Medium,
    /// 深度分析、多文件、多步骤、长上下文
    Reasoning,
    /// 需要并行协作、子 agent 编排或多工作流协调
    Complex,
}

/// Provider 评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderScore {
    pub provider_id: String,
    pub model_id: String,
    /// 综合分数 0.0-1.0
    pub total_score: f64,
    /// 质量分数
    pub quality_score: f64,
    /// 成本分数（越低越好，0.0 = 最贵，1.0 = 最便宜）
    pub cost_score: f64,
    /// 延迟分数（越低越好，0.0 = 最慢，1.0 = 最快）
    pub latency_score: f64,
    /// 可用性分数
    pub availability_score: f64,
    /// 预估成本 (USD/token)
    pub estimated_cost_per_1k: f64,
    /// USD / 1K input tokens
    pub input_cost_per_1k: f64,
    /// USD / 1K output tokens
    pub output_cost_per_1k: f64,
    /// 平均延迟 (ms)
    pub avg_latency_ms: u64,
    /// 当前可用性状态
    pub availability: AvailabilityStatus,
    /// Policy adjustments that affected this candidate.
    pub policy_reason: Option<String>,
    /// Model-level fallback worker certification profile.
    pub handoff_certification: HandoffCertification,
    /// Weighted score components used to explain the final total score.
    pub score_breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub weighted_quality: f64,
    pub weighted_cost: f64,
    pub weighted_latency: f64,
    pub weighted_availability: f64,
    /// Explicit user policy adjustment from configured preferred/avoided model keywords.
    pub weighted_policy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    pub provider_id: String,
    pub sample_count: u64,
    pub success_rate: f64,
    pub avg_latency_ms: u64,
}

/// 路由决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// 选中的 provider
    pub provider_id: String,
    /// 实际使用的 model ID
    pub model_id: String,
    /// 路由策略
    pub strategy: RoutingStrategy,
    /// 面向 AI 编程代理的任务层级
    pub agent_tier: AgentTier,
    /// 所有候选的评分
    pub candidates: Vec<ProviderScore>,
    /// 决策耗时 (ms)
    pub decision_time_ms: u64,
    /// 是否走了 fallback
    pub is_fallback: bool,
    /// fallback 原因
    pub fallback_reason: Option<String>,
    /// Ordered provider/model attempts made while serving this request.
    #[serde(default)]
    pub fallback_chain: Vec<RouteAttempt>,
    /// Policy reason from the selected candidate, when applicable.
    pub policy_reason: Option<String>,
    /// Human-readable explanation for why this route was selected.
    pub decision_reason: String,
    /// Candidates excluded before scoring and why they were excluded.
    pub excluded_candidates: Vec<ExcludedCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAttempt {
    pub provider_id: String,
    pub model_id: String,
    pub status: RouteAttemptStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteAttemptStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludedCandidate {
    pub provider_id: String,
    pub model_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// 质量优先
    Auto,
    /// 成本优先
    Save,
    /// 延迟优先
    Fast,
    /// 手动指定
    Manual,
    /// 随机负载均衡
    Random,
}

/// 路由配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub default_strategy: RoutingStrategy,
    pub quality_weight: f64,
    pub cost_weight: f64,
    pub latency_weight: f64,
    pub fallback_enabled: bool,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_secs: u64,
    /// 各策略的权重配置
    pub strategy_weights: HashMap<String, StrategyWeights>,
    /// Per-tier routing policy overrides.
    pub tier_policies: HashMap<AgentTier, TierRoutingPolicy>,
    /// Strategy names explicitly declared by the loaded config source.
    #[serde(skip)]
    pub configured_strategy_weights: HashSet<String>,
    /// Tiers explicitly declared by the loaded config source.
    #[serde(skip)]
    pub configured_tier_policies: HashSet<AgentTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyWeights {
    pub quality: f64,
    pub cost: f64,
    pub latency: f64,
    pub availability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TierRoutingPolicy {
    pub quality_weight: Option<f64>,
    pub cost_weight: Option<f64>,
    pub latency_weight: Option<f64>,
    #[serde(default)]
    pub preferred_model_keywords: Vec<String>,
    #[serde(default)]
    pub avoided_model_keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialRoutingConfig {
    default_strategy: Option<RoutingStrategy>,
    quality_weight: Option<f64>,
    cost_weight: Option<f64>,
    latency_weight: Option<f64>,
    fallback_enabled: Option<bool>,
    circuit_breaker_threshold: Option<u32>,
    circuit_breaker_timeout_secs: Option<u64>,
    strategy_weights: Option<HashMap<String, StrategyWeights>>,
    tier_policies: Option<HashMap<AgentTier, TierRoutingPolicy>>,
}

impl RoutingConfig {
    pub fn from_toml_str(input: &str) -> anyhow::Result<Self> {
        let partial: PartialRoutingConfig = toml::from_str(input)?;
        let mut config = Self::default();

        if let Some(value) = partial.default_strategy {
            config.default_strategy = value;
        }
        if let Some(value) = partial.quality_weight {
            config.quality_weight = value;
        }
        if let Some(value) = partial.cost_weight {
            config.cost_weight = value;
        }
        if let Some(value) = partial.latency_weight {
            config.latency_weight = value;
        }
        if let Some(value) = partial.fallback_enabled {
            config.fallback_enabled = value;
        }
        if let Some(value) = partial.circuit_breaker_threshold {
            config.circuit_breaker_threshold = value;
        }
        if let Some(value) = partial.circuit_breaker_timeout_secs {
            config.circuit_breaker_timeout_secs = value;
        }
        if let Some(value) = partial.strategy_weights {
            config
                .configured_strategy_weights
                .extend(value.keys().cloned());
            config.strategy_weights.extend(value);
        }
        if let Some(value) = partial.tier_policies {
            config
                .configured_tier_policies
                .extend(value.keys().copied());
            config.tier_policies.extend(value);
        }

        Ok(config)
    }

    pub fn from_toml_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }
}

impl Default for RoutingConfig {
    fn default() -> Self {
        let mut strategy_weights = HashMap::new();
        strategy_weights.insert(
            "auto".to_string(),
            StrategyWeights {
                quality: 0.5,
                cost: 0.2,
                latency: 0.15,
                availability: 0.15,
            },
        );
        strategy_weights.insert(
            "save".to_string(),
            StrategyWeights {
                quality: 0.2,
                cost: 0.6,
                latency: 0.1,
                availability: 0.1,
            },
        );
        strategy_weights.insert(
            "fast".to_string(),
            StrategyWeights {
                quality: 0.25,
                cost: 0.1,
                latency: 0.55,
                availability: 0.1,
            },
        );

        Self {
            default_strategy: RoutingStrategy::Auto,
            quality_weight: 0.5,
            cost_weight: 0.2,
            latency_weight: 0.15,
            fallback_enabled: true,
            circuit_breaker_threshold: 3,
            circuit_breaker_timeout_secs: 30,
            strategy_weights,
            tier_policies: HashMap::new(),
            configured_strategy_weights: HashSet::new(),
            configured_tier_policies: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_config_loads_partial_tier_policy_from_toml() {
        let config = RoutingConfig::from_toml_str(
            r#"
fallback_enabled = false

[tier_policies.simple]
quality_weight = 0.1
cost_weight = 0.65
latency_weight = 0.15
preferred_model_keywords = ["flash", "haiku"]
avoided_model_keywords = ["opus"]
"#,
        )
        .unwrap();

        assert!(!config.fallback_enabled);
        assert_eq!(config.default_strategy, RoutingStrategy::Auto);
        assert_eq!(config.strategy_weights["auto"].quality, 0.5);

        let simple = config.tier_policies.get(&AgentTier::Simple).unwrap();
        assert_eq!(simple.quality_weight, Some(0.1));
        assert_eq!(simple.cost_weight, Some(0.65));
        assert_eq!(simple.latency_weight, Some(0.15));
        assert_eq!(simple.preferred_model_keywords, vec!["flash", "haiku"]);
        assert_eq!(simple.avoided_model_keywords, vec!["opus"]);
    }

    #[test]
    fn routing_config_loads_from_toml_file() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-config-test-{}.toml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
[tier_policies.reasoning]
quality_weight = 0.75
preferred_model_keywords = ["sonnet", "gpt"]
"#,
        )
        .unwrap();

        let config = RoutingConfig::from_toml_file(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let reasoning = config.tier_policies.get(&AgentTier::Reasoning).unwrap();
        assert_eq!(reasoning.quality_weight, Some(0.75));
        assert_eq!(reasoning.preferred_model_keywords, vec!["sonnet", "gpt"]);
    }
}

/// Provider 能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapability {
    pub provider_id: String,
    pub provider_name: String,
    pub supported_models: Vec<ModelInfo>,
    pub base_url: String,
    pub requires_api_key: bool,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub max_context_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_id: String,
    pub display_name: String,
    /// USD / 1K input tokens
    pub input_cost_per_1k: f64,
    /// USD / 1K output tokens
    pub output_cost_per_1k: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing_updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_streaming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_vision: Option<bool>,
    pub max_tokens: u32,
    /// 质量评分 0.0-1.0
    pub quality_score: f64,
    pub avg_latency_ms: u64,
    #[serde(default)]
    pub handoff_certification: HandoffCertification,
}

/// Provider trait — 所有 LLM provider 必须实现
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn capability(&self) -> &ProviderCapability;

    /// 检查 provider 当前是否可用
    async fn health_check(&self) -> Result<bool, ProviderError>;

    /// 发送 chat completion 请求，返回响应流
    async fn chat_completion(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    /// 发送 streaming chat completion
    async fn chat_completion_stream(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
        ProviderError,
    >;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub provider: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    pub finish_reason: Option<String>,
    /// 耗时 (ms)
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<StreamChoice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider unavailable: {0}")]
    Unavailable(String),
    #[error("Rate limited: {0}")]
    RateLimited(String),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Provider error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub provider_id: String,
    pub model_id: String,
    pub request_tokens: u32,
    pub response_tokens: u32,
    pub latency_ms: u64,
    pub cost_usd: f64,
    pub success: bool,
    pub error_message: Option<String>,
    pub routing_strategy: String,
    pub agent_tier: String,
    pub is_fallback: bool,
}
