use crate::http_client::build_client;
use dispatcher_engine::types::*;
use reqwest::Client;

pub struct OllamaProvider {
    base_url: String,
    client: Client,
    capability: ProviderCapability,
}

impl OllamaProvider {
    pub fn new(base_url: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(300)).unwrap();

        let capability = ProviderCapability {
            provider_id: "ollama".into(),
            provider_name: "Ollama (Local)".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "llama3".into(),
                    display_name: "Llama 3".into(),
                    input_cost_per_1k: 0.0,
                    output_cost_per_1k: 0.0,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 8192,
                    quality_score: 0.70,
                    avg_latency_ms: 3000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "qwen2.5".into(),
                    display_name: "Qwen 2.5".into(),
                    input_cost_per_1k: 0.0,
                    output_cost_per_1k: 0.0,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 32768,
                    quality_score: 0.68,
                    avg_latency_ms: 2500,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: base_url.clone(),
            requires_api_key: false,
            supports_streaming: false,
            supports_tools: false,
            supports_vision: false,
            max_context_length: 8192,
        };

        Self {
            base_url,
            client,
            capability,
        }
    }
}

#[async_trait::async_trait]
impl Provider for OllamaProvider {
    fn provider_id(&self) -> &str {
        "ollama"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await;

        match resp {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    async fn chat_completion(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let start = std::time::Instant::now();

        let messages: Vec<_> = request
            .messages
            .iter()
            .map(|m| {
                let content = match &m.content {
                    MessageContent::Text(t) => t.clone(),
                    MessageContent::MultiPart(parts) => parts
                        .iter()
                        .map(|p| p.text.clone().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(""),
                };
                serde_json::json!({
                    "role": m.role,
                    "content": content,
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": model_id,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7),
                "num_predict": request.max_tokens.unwrap_or(4096),
            },
        });

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let latency_ms = start.elapsed().as_millis() as u64;

        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Other(format!("Ollama error: {}", err_text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let content = json["message"]["content"].as_str().unwrap_or("");
        let eval_count = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
        let prompt_eval_count = json
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(ChatCompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: model_id.into(),
            provider: "ollama".into(),
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: content.into(),
                    tool_calls: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Usage {
                prompt_tokens: prompt_eval_count as u32,
                completion_tokens: eval_count as u32,
                total_tokens: (prompt_eval_count + eval_count) as u32,
            },
            finish_reason: Some("stop".into()),
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
            "Streaming not yet implemented for Ollama".into(),
        ))
    }
}
