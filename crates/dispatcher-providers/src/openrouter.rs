use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct OpenRouterProvider {
    api_key: String,
    client: Client,
    capability: ProviderCapability,
}

impl OpenRouterProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "openrouter".into(),
            provider_name: "OpenRouter".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "openai/gpt-4o".into(),
                    display_name: "OpenRouter GPT-4o".into(),
                    input_cost_per_1k: 0.0025,
                    output_cost_per_1k: 0.01,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 128_000,
                    quality_score: 0.92,
                    avg_latency_ms: 1800,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "anthropic/claude-sonnet-4-6".into(),
                    display_name: "OpenRouter Claude Sonnet 4.6".into(),
                    input_cost_per_1k: 0.003,
                    output_cost_per_1k: 0.015,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 200_000,
                    quality_score: 0.93,
                    avg_latency_ms: 2200,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "google/gemini-2.5-flash".into(),
                    display_name: "OpenRouter Gemini Flash".into(),
                    input_cost_per_1k: 0.00015,
                    output_cost_per_1k: 0.0006,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 1_048_576,
                    quality_score: 0.85,
                    avg_latency_ms: 700,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: "https://openrouter.ai/api".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            max_context_length: 200_000,
        };

        Self {
            api_key,
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for OpenRouterProvider {
    fn provider_id(&self) -> &str {
        "openrouter"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let resp = self
            .client
            .get("https://openrouter.ai/api/v1/models")
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
        let start = std::time::Instant::now();

        let body = crate::openai_compat::build_openai_compat_body(request, model_id, false);

        let resp = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://dispatcher.local")
            .header("X-Title", "Dispatcher")
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
            "openrouter",
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
        crate::openai_compat::stream_openai_compat(
            &self.client,
            "https://openrouter.ai/api/v1/chat/completions",
            &self.api_key,
            model_id,
            request,
        )
        .await
    }
}
