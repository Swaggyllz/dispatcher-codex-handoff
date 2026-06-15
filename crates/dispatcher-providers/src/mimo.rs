use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct MiMoProvider {
    api_key: String,
    client: Client,
    capability: ProviderCapability,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn mimo_text_prices_match_current_overseas_cache_miss_usd_rates() {
        let provider = MiMoProvider::new("test-key".into());
        let find_model = |id: &str| {
            provider
                .capability()
                .supported_models
                .iter()
                .find(|model| model.model_id == id)
                .unwrap()
        };

        let v25_pro = find_model("mimo-v2.5-pro");
        let v25 = find_model("mimo-v2.5");
        let flash = find_model("mimo-v2-flash");

        assert!((v25_pro.input_cost_per_1k - 0.001).abs() < f64::EPSILON);
        assert!((v25_pro.output_cost_per_1k - 0.003).abs() < f64::EPSILON);
        assert!((v25.input_cost_per_1k - 0.0004).abs() < f64::EPSILON);
        assert!((v25.output_cost_per_1k - 0.002).abs() < f64::EPSILON);
        assert!((flash.input_cost_per_1k - 0.0001).abs() < f64::EPSILON);
        assert!((flash.output_cost_per_1k - 0.0003).abs() < f64::EPSILON);
    }
}

impl MiMoProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "mimo".into(),
            provider_name: "小米 MiMo".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "mimo-v2.5-pro".into(),
                    display_name: "MiMo V2.5 Pro".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.003,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 262144,
                    quality_score: 0.88,
                    avg_latency_ms: 2500,
                },
                ModelInfo {
                    model_id: "mimo-v2.5".into(),
                    display_name: "MiMo V2.5".into(),
                    input_cost_per_1k: 0.0004,
                    output_cost_per_1k: 0.002,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 262144,
                    quality_score: 0.85,
                    avg_latency_ms: 1000,
                },
                ModelInfo {
                    model_id: "mimo-v2-pro".into(),
                    display_name: "MiMo V2 Pro".into(),
                    input_cost_per_1k: 0.001,
                    output_cost_per_1k: 0.003,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 262144,
                    quality_score: 0.86,
                    avg_latency_ms: 2000,
                },
                ModelInfo {
                    model_id: "mimo-v2-flash".into(),
                    display_name: "MiMo V2 Flash".into(),
                    input_cost_per_1k: 0.0001,
                    output_cost_per_1k: 0.0003,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 262144,
                    quality_score: 0.82,
                    avg_latency_ms: 600,
                },
            ],
            base_url: "https://api.xiaomimimo.com".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: false,
            supports_vision: false,
            max_context_length: 262144,
        };

        Self {
            api_key,
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for MiMoProvider {
    fn provider_id(&self) -> &str {
        "mimo"
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
            json, "mimo", model_id, latency_ms,
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
