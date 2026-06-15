use dispatcher_engine::types::*;

pub struct DemoProvider {
    capability: ProviderCapability,
}

impl Default for DemoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoProvider {
    pub fn new() -> Self {
        Self {
            capability: ProviderCapability {
                provider_id: "demo".into(),
                provider_name: "Demo Provider (Local)".into(),
                supported_models: vec![ModelInfo {
                    model_id: "demo-echo".into(),
                    display_name: "Demo Echo".into(),
                    input_cost_per_1k: 0.0,
                    output_cost_per_1k: 0.0,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 8192,
                    quality_score: 0.72,
                    avg_latency_ms: 25,
                }],
                base_url: "local://demo".into(),
                requires_api_key: false,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: false,
                max_context_length: 8192,
            },
        }
    }

    fn latest_user_text(request: &ModelRequest) -> String {
        request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| match &message.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::MultiPart(parts) => parts
                    .iter()
                    .filter_map(|part| part.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl Provider for DemoProvider {
    fn provider_id(&self) -> &str {
        "demo"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        Ok(true)
    }

    async fn chat_completion(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let prompt = Self::latest_user_text(request);
        let content = format!(
            "[demo] Dispatcher received your prompt and routed it locally.\n\nPrompt: {}",
            prompt
        );
        let prompt_tokens = (prompt.chars().count() / 4).max(1) as u32;
        let completion_tokens = (content.chars().count() / 4).max(1) as u32;

        Ok(ChatCompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: model_id.into(),
            provider: "demo".into(),
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".into(),
                    content,
                    tool_calls: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            finish_reason: Some("stop".into()),
            latency_ms: 1,
        })
    }

    async fn chat_completion_stream(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
        ProviderError,
    > {
        let response = self.chat_completion(request, model_id).await?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default();
        let chunks = vec![
            Ok(StreamChunk {
                id: response.id.clone(),
                model: response.model.clone(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        role: Some("assistant".into()),
                        content: Some(content),
                        reasoning_content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
                usage: None,
            }),
            Ok(StreamChunk {
                id: response.id,
                model: response.model,
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: None,
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: Some(response.usage),
            }),
        ];

        Ok(Box::new(futures::stream::iter(chunks)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(text: &str) -> ModelRequest {
        ModelRequest {
            model: "auto".into(),
            messages: vec![Message {
                role: "user".into(),
                content: MessageContent::Text(text.into()),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: None,
            extra: Default::default(),
        }
    }

    #[tokio::test]
    async fn demo_provider_returns_local_echo_completion() {
        let provider = DemoProvider::new();
        let response = provider
            .chat_completion(&request("hello dispatcher"), "demo-echo")
            .await
            .unwrap();

        assert_eq!(response.provider, "demo");
        assert_eq!(response.model, "demo-echo");
        assert!(response.choices[0]
            .message
            .content
            .contains("hello dispatcher"));
    }

    #[tokio::test]
    async fn demo_provider_supports_streaming_for_local_client_tests() {
        use futures::StreamExt;

        let provider = DemoProvider::new();
        let mut stream = provider
            .chat_completion_stream(&request("hello stream"), "demo-echo")
            .await
            .unwrap();
        let chunks: Vec<_> = stream.by_ref().collect().await;

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(Result::is_ok));
        assert!(chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .filter_map(|chunk| chunk.choices.first())
            .filter_map(|choice| choice.delta.content.as_deref())
            .collect::<String>()
            .contains("hello stream"));
        assert!(chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .any(|chunk| chunk.usage.is_some()));
    }
}
