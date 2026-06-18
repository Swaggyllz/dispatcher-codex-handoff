use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct GeminiProvider {
    api_key: String,
    client: Client,
    capability: ProviderCapability,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "gemini".into(),
            provider_name: "Google Gemini".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "gemini-2.5-flash".into(),
                    display_name: "Gemini 2.5 Flash".into(),
                    input_cost_per_1k: 0.00015,
                    output_cost_per_1k: 0.0006,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 1_048_576,
                    quality_score: 0.85,
                    avg_latency_ms: 500,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "gemini-2.5-pro".into(),
                    display_name: "Gemini 2.5 Pro".into(),
                    input_cost_per_1k: 0.00125,
                    output_cost_per_1k: 0.005,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 1_048_576,
                    quality_score: 0.91,
                    avg_latency_ms: 1500,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: "https://generativelanguage.googleapis.com".into(),
            requires_api_key: true,
            supports_streaming: false,
            supports_tools: true,
            supports_vision: true,
            max_context_length: 1_048_576,
        };

        Self {
            api_key,
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for GeminiProvider {
    fn provider_id(&self) -> &str {
        "gemini"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            self.api_key
        );
        let resp = self
            .client
            .get(&url)
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
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model_id, self.api_key
        );

        // 转换 messages 到 Gemini 格式
        let contents: Vec<_> = request
            .messages
            .iter()
            .map(|m| {
                let text = match &m.content {
                    MessageContent::Text(t) => t.clone(),
                    MessageContent::MultiPart(parts) => parts
                        .iter()
                        .map(|p| p.text.clone().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(""),
                };
                let role = match m.role.as_str() {
                    "assistant" => "model",
                    _other => "user",
                };
                serde_json::json!({
                    "role": role,
                    "parts": [{"text": text}],
                })
            })
            .collect();

        let body = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "temperature": request.temperature.unwrap_or(0.7),
                "maxOutputTokens": request.max_tokens.unwrap_or(4096),
            },
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let latency_ms = start.elapsed().as_millis() as u64;

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
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

        let candidate = &json["candidates"][0];
        let content = candidate["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("");
        let usage_meta = &json["usageMetadata"];

        Ok(ChatCompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: model_id.into(),
            provider: "gemini".into(),
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: content.into(),
                    tool_calls: None,
                },
                finish_reason: candidate["finishReason"].as_str().map(|s| s.into()),
            }],
            usage: Usage {
                prompt_tokens: usage_meta["promptTokenCount"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_meta["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
                total_tokens: usage_meta["totalTokenCount"].as_u64().unwrap_or(0) as u32,
            },
            finish_reason: candidate["finishReason"].as_str().map(|s| s.into()),
            latency_ms,
        })
    }

    async fn chat_completion_stream(
        &self,
        _request: &ModelRequest,
        _model_id: &str,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
        ProviderError,
    > {
        Err(ProviderError::Other(
            "Streaming not yet implemented for Gemini".into(),
        ))
    }
}
