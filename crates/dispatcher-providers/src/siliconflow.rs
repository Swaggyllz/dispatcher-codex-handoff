use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct SiliconFlowProvider {
    api_key: String,
    client: Client,
    capability: ProviderCapability,
}

impl SiliconFlowProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "siliconflow".into(),
            provider_name: "硅基流动 SiliconFlow".into(),
            supported_models: vec![
                // Qwen 系列
                ModelInfo {
                    model_id: "Qwen/Qwen3.5-122B-A10B".into(),
                    display_name: "Qwen 3.5 122B".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.91,
                    avg_latency_ms: 3000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen3-32B".into(),
                    display_name: "Qwen 3 32B".into(),
                    input_cost_per_1k: 0.0005,
                    output_cost_per_1k: 0.001,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.87,
                    avg_latency_ms: 1500,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen3-14B".into(),
                    display_name: "Qwen 3 14B".into(),
                    input_cost_per_1k: 0.0003,
                    output_cost_per_1k: 0.0006,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.82,
                    avg_latency_ms: 800,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen3-8B".into(),
                    display_name: "Qwen 3 8B".into(),
                    input_cost_per_1k: 0.00015,
                    output_cost_per_1k: 0.0003,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.72,
                    avg_latency_ms: 500,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen3-Coder-30B-A3B-Instruct".into(),
                    display_name: "Qwen 3 Coder 30B".into(),
                    input_cost_per_1k: 0.0005,
                    output_cost_per_1k: 0.001,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.86,
                    avg_latency_ms: 1200,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen2.5-72B-Instruct".into(),
                    display_name: "Qwen 2.5 72B".into(),
                    input_cost_per_1k: 0.0005,
                    output_cost_per_1k: 0.001,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 32768,
                    quality_score: 0.83,
                    avg_latency_ms: 1200,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen2.5-32B-Instruct".into(),
                    display_name: "Qwen 2.5 32B".into(),
                    input_cost_per_1k: 0.0003,
                    output_cost_per_1k: 0.0006,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 32768,
                    quality_score: 0.80,
                    avg_latency_ms: 900,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen2.5-14B-Instruct".into(),
                    display_name: "Qwen 2.5 14B".into(),
                    input_cost_per_1k: 0.0002,
                    output_cost_per_1k: 0.0004,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 32768,
                    quality_score: 0.76,
                    avg_latency_ms: 600,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Qwen/Qwen2.5-7B-Instruct".into(),
                    display_name: "Qwen 2.5 7B".into(),
                    input_cost_per_1k: 0.0001,
                    output_cost_per_1k: 0.0001,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 32768,
                    quality_score: 0.65,
                    avg_latency_ms: 400,
                    handoff_certification: HandoffCertification::default(),
                },
                // DeepSeek 系列
                ModelInfo {
                    model_id: "Pro/deepseek-ai/DeepSeek-V3.2".into(),
                    display_name: "DeepSeek V3.2".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 65536,
                    quality_score: 0.90,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/deepseek-ai/DeepSeek-V3.1-Terminus".into(),
                    display_name: "DeepSeek V3.1".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 65536,
                    quality_score: 0.89,
                    avg_latency_ms: 1800,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/deepseek-ai/DeepSeek-V3".into(),
                    display_name: "DeepSeek V3".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 65536,
                    quality_score: 0.88,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/deepseek-ai/DeepSeek-R1".into(),
                    display_name: "DeepSeek R1".into(),
                    input_cost_per_1k: 0.004,
                    output_cost_per_1k: 0.016,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 65536,
                    quality_score: 0.92,
                    avg_latency_ms: 4000,
                    handoff_certification: HandoffCertification::default(),
                },
                // Kimi 系列
                ModelInfo {
                    model_id: "Pro/moonshotai/Kimi-K2.6".into(),
                    display_name: "Kimi K2.6".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.89,
                    avg_latency_ms: 1800,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/moonshotai/Kimi-K2.5".into(),
                    display_name: "Kimi K2.5".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.87,
                    avg_latency_ms: 1800,
                    handoff_certification: HandoffCertification::default(),
                },
                // GLM 系列
                ModelInfo {
                    model_id: "Pro/zai-org/GLM-5.1".into(),
                    display_name: "GLM 5.1".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.90,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/zai-org/GLM-5".into(),
                    display_name: "GLM 5".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.88,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "Pro/zai-org/GLM-4.7".into(),
                    display_name: "GLM 4.7".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.86,
                    avg_latency_ms: 1800,
                    handoff_certification: HandoffCertification::default(),
                },
                // MiniMax
                ModelInfo {
                    model_id: "MiniMaxAI/MiniMax-M2.5".into(),
                    display_name: "MiniMax M2.5".into(),
                    input_cost_per_1k: 0.0005,
                    output_cost_per_1k: 0.001,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.84,
                    avg_latency_ms: 1200,
                    handoff_certification: HandoffCertification::default(),
                },
                // ByteDance
                ModelInfo {
                    model_id: "ByteDance-Seed/Seed-OSS-36B-Instruct".into(),
                    display_name: "Seed OSS 36B".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 131072,
                    quality_score: 0.85,
                    avg_latency_ms: 1500,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: "https://api.siliconflow.cn".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: false,
            max_context_length: 65536,
        };

        Self {
            api_key,
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for SiliconFlowProvider {
    fn provider_id(&self) -> &str {
        "siliconflow"
    }
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1/models", self.capability.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(resp.status().is_success())
    }

    async fn chat_completion(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.capability.base_url);
        let start = std::time::Instant::now();

        let body = crate::openai_compat::build_openai_compat_body(request, model_id, false);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let latency_ms = start.elapsed().as_millis() as u64;

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthFailed("Invalid API key".into()));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited("Rate limit exceeded".into()));
        }
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Other(format!(
                "HTTP {}: {}",
                status, err_text
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        Ok(crate::openai_compat::parse_openai_compat_response(
            json,
            "siliconflow",
            model_id,
            latency_ms,
        ))
    }

    async fn chat_completion_stream(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
        ProviderError,
    > {
        let url = format!("{}/v1/chat/completions", self.capability.base_url);
        crate::openai_compat::stream_openai_compat(
            &self.client,
            &url,
            &self.api_key,
            model_id,
            request,
        )
        .await
    }
}
