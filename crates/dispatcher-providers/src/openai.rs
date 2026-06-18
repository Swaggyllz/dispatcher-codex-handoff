use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    client: Client,
    capability: ProviderCapability,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "openai".into(),
            provider_name: "OpenAI".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "gpt-4o".into(),
                    display_name: "GPT-4o".into(),
                    input_cost_per_1k: 0.0025,
                    output_cost_per_1k: 0.01,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 128_000,
                    quality_score: 0.92,
                    avg_latency_ms: 1500,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "gpt-4o-mini".into(),
                    display_name: "GPT-4o Mini".into(),
                    input_cost_per_1k: 0.00015,
                    output_cost_per_1k: 0.0006,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 128_000,
                    quality_score: 0.78,
                    avg_latency_ms: 800,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "gpt-4-turbo".into(),
                    display_name: "GPT-4 Turbo".into(),
                    input_cost_per_1k: 0.01,
                    output_cost_per_1k: 0.03,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 128_000,
                    quality_score: 0.90,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: "https://api.openai.com".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            max_context_length: 128_000,
        };

        Self {
            api_key,
            base_url: "https://api.openai.com".into(),
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for OpenAIProvider {
    fn provider_id(&self) -> &str {
        "openai"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1/models", self.base_url))
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
        let url = format!("{}/v1/chat/completions", self.base_url);
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
            json, "openai", model_id, latency_ms,
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
        let url = format!("{}/v1/chat/completions", self.base_url);
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
