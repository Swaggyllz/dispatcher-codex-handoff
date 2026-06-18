use crate::http_client::build_client;
use dispatcher_engine::types::*;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
    capability: ProviderCapability,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        let client = build_client(std::time::Duration::from_secs(120)).unwrap();

        let capability = ProviderCapability {
            provider_id: "anthropic".into(),
            provider_name: "Anthropic".into(),
            supported_models: vec![
                ModelInfo {
                    model_id: "claude-sonnet-4-6".into(),
                    display_name: "Claude Sonnet 4.6".into(),
                    input_cost_per_1k: 0.003,
                    output_cost_per_1k: 0.015,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 200_000,
                    quality_score: 0.93,
                    avg_latency_ms: 2000,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "claude-haiku-4-5".into(),
                    display_name: "Claude Haiku 4.5".into(),
                    input_cost_per_1k: 0.0008,
                    output_cost_per_1k: 0.004,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 200_000,
                    quality_score: 0.82,
                    avg_latency_ms: 600,
                    handoff_certification: HandoffCertification::default(),
                },
                ModelInfo {
                    model_id: "claude-opus-4-7".into(),
                    display_name: "Claude Opus 4.7".into(),
                    input_cost_per_1k: 0.015,
                    output_cost_per_1k: 0.075,
                    pricing_source: None,
                    pricing_updated_at: None,
                    supports_streaming: None,
                    supports_tools: None,
                    supports_vision: None,
                    max_tokens: 200_000,
                    quality_score: 0.96,
                    avg_latency_ms: 4000,
                    handoff_certification: HandoffCertification::default(),
                },
            ],
            base_url: "https://api.anthropic.com".into(),
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

fn build_anthropic_body(request: &ModelRequest, model_id: &str, stream: bool) -> serde_json::Value {
    let system = request
        .messages
        .iter()
        .filter(|message| message.role == "system")
        .filter_map(|message| match &message.content {
            MessageContent::Text(text) => Some(text.clone()),
            MessageContent::MultiPart(parts) => {
                let text = parts
                    .iter()
                    .filter_map(|part| part.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");
                (!text.is_empty()).then_some(text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let messages: Vec<_> = request
        .messages
        .iter()
        .filter(|message| message.role != "system")
        .map(|message| {
            if let MessageContent::Text(text) = &message.content {
                if message.role == "assistant" {
                    if let Some((name, call_id, arguments)) =
                        crate::openai_compat::parse_tool_call_marker(text)
                    {
                        return serde_json::json!({
                            "role": "assistant",
                            "content": [{
                                "type": "tool_use",
                                "id": call_id,
                                "name": name,
                                "input": serde_json::from_str::<serde_json::Value>(arguments)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                            }],
                        });
                    }
                }
                if message.role == "user" {
                    if let Some((call_id, output)) =
                        crate::openai_compat::parse_tool_result_marker(text)
                    {
                        return serde_json::json!({
                            "role": "user",
                            "content": [{
                                "type": "tool_result",
                                "tool_use_id": call_id,
                                "content": output,
                            }],
                        });
                    }
                }
            }

            let content = match &message.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::MultiPart(parts) => parts
                    .iter()
                    .filter_map(|part| part.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            serde_json::json!({
                "role": message.role,
                "content": content,
            })
        })
        .collect();

    let tools: Vec<_> = request
        .tools
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.function.name,
                "description": tool.function.description,
                "input_schema": tool.function.parameters.clone().unwrap_or_else(|| {
                    serde_json::json!({"type": "object", "properties": {}})
                }),
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": model_id,
        "messages": messages,
        "max_tokens": request.max_tokens.unwrap_or(4096),
        "stream": stream,
    });
    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system);
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = serde_json::json!(temperature);
    }
    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools);
    }
    body
}

fn parse_anthropic_response(
    json: serde_json::Value,
    fallback_model: &str,
    latency_ms: u64,
) -> ChatCompletionResponse {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for block in json["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => content.push_str(block["text"].as_str().unwrap_or("")),
            Some("tool_use") => tool_calls.push(ToolCall {
                index: Some(tool_calls.len() as u32),
                id: block["id"].as_str().unwrap_or("").into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: block["name"].as_str().unwrap_or("").into(),
                    arguments: block["input"].to_string(),
                },
            }),
            _ => {}
        }
    }
    let usage = &json["usage"];
    let prompt_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
    let completion_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as u32;
    let finish_reason = json["stop_reason"].as_str().map(str::to_string);

    ChatCompletionResponse {
        id: json["id"].as_str().unwrap_or("").into(),
        model: json["model"].as_str().unwrap_or(fallback_model).into(),
        provider: "anthropic".into(),
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".into(),
                content,
                tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
            },
            finish_reason: finish_reason.clone(),
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        finish_reason,
        latency_ms,
    }
}

#[derive(Default)]
struct AnthropicSseDecoder {
    buffer: String,
    message_id: String,
    model: String,
    prompt_tokens: u32,
    tool_blocks: HashMap<u32, (String, String)>,
}

impl AnthropicSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<StreamChunk>, ProviderError> {
        self.buffer
            .push_str(&String::from_utf8_lossy(bytes).replace("\r\n", "\n"));
        let mut chunks = Vec::new();

        while let Some(boundary) = self.buffer.find("\n\n") {
            let event = self.buffer[..boundary].to_string();
            self.buffer.drain(..boundary + 2);
            let data = event
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() {
                continue;
            }

            let value: serde_json::Value = serde_json::from_str(&data)
                .map_err(|error| ProviderError::Other(error.to_string()))?;
            if let Some(chunk) = self.decode_event(&value) {
                chunks.push(chunk);
            }
        }

        Ok(chunks)
    }

    fn decode_event(&mut self, value: &serde_json::Value) -> Option<StreamChunk> {
        match value["type"].as_str()? {
            "message_start" => {
                self.message_id = value["message"]["id"].as_str().unwrap_or("").into();
                self.model = value["message"]["model"].as_str().unwrap_or("").into();
                self.prompt_tokens = value["message"]["usage"]["input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                Some(self.chunk(
                    StreamDelta {
                        role: Some("assistant".into()),
                        content: None,
                        reasoning_content: None,
                        tool_calls: None,
                    },
                    None,
                    None,
                ))
            }
            "content_block_delta" => {
                match value["delta"]["type"].as_str()? {
                    "text_delta" => {
                        let text = value["delta"]["text"].as_str()?.to_string();
                        Some(self.chunk(
                            StreamDelta {
                                role: None,
                                content: Some(text),
                                reasoning_content: None,
                                tool_calls: None,
                            },
                            None,
                            None,
                        ))
                    }
                    "input_json_delta" => {
                        let index = value["index"].as_u64().unwrap_or(0) as u32;
                        let (id, _) = self.tool_blocks.get(&index)?.clone();
                        Some(self.chunk(
                            StreamDelta {
                                role: None,
                                content: None,
                                reasoning_content: None,
                                tool_calls: Some(vec![ToolCall {
                                    index: Some(index),
                                    id,
                                    call_type: "function".into(),
                                    function: FunctionCall {
                                        name: String::new(),
                                        arguments: value["delta"]["partial_json"]
                                            .as_str()
                                            .unwrap_or("")
                                            .into(),
                                    },
                                }]),
                            },
                            None,
                            None,
                        ))
                    }
                    _ => None,
                }
            }
            "content_block_start" if value["content_block"]["type"] == "tool_use" => {
                let index = value["index"].as_u64().unwrap_or(0) as u32;
                let id = value["content_block"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let name = value["content_block"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                self.tool_blocks.insert(index, (id.clone(), name.clone()));
                Some(self.chunk(
                    StreamDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: Some(vec![ToolCall {
                            index: Some(index),
                            id,
                            call_type: "function".into(),
                            function: FunctionCall {
                                name,
                                arguments: String::new(),
                            },
                        }]),
                    },
                    None,
                    None,
                ))
            }
            "message_delta" => {
                let completion_tokens =
                    value["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
                let finish_reason = value["delta"]["stop_reason"]
                    .as_str()
                    .map(ToString::to_string);
                Some(self.chunk(
                    StreamDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: None,
                    },
                    finish_reason,
                    Some(Usage {
                        prompt_tokens: self.prompt_tokens,
                        completion_tokens,
                        total_tokens: self.prompt_tokens + completion_tokens,
                    }),
                ))
            }
            _ => None,
        }
    }

    fn chunk(
        &self,
        delta: StreamDelta,
        finish_reason: Option<String>,
        usage: Option<Usage>,
    ) -> StreamChunk {
        StreamChunk {
            id: self.message_id.clone(),
            model: self.model.clone(),
            choices: vec![StreamChoice {
                index: 0,
                delta,
                finish_reason,
            }],
            usage,
        }
    }
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        "anthropic"
    }

    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    async fn health_check(&self) -> Result<bool, ProviderError> {
        let resp = self
            .client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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

        let body = build_anthropic_body(request, model_id, false);

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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

        Ok(parse_anthropic_response(json, model_id, latency_ms))
    }

    async fn chat_completion_stream(
        &self,
        request: &ModelRequest,
        model_id: &str,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
        ProviderError,
    > {
        let body = build_anthropic_body(request, model_id, true);
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|error| ProviderError::Network(error.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthFailed("Invalid API key".into()));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited("Rate limit exceeded".into()));
        }
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Other(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let stream = response
            .bytes_stream()
            .scan(AnthropicSseDecoder::default(), |decoder, result| {
                let decoded = match result {
                    Ok(bytes) => decoder.push(&bytes),
                    Err(error) => Err(ProviderError::Network(error.to_string())),
                };
                std::future::ready(Some(decoded))
            })
            .flat_map(|decoded| {
                let items = match decoded {
                    Ok(chunks) => chunks.into_iter().map(Ok).collect(),
                    Err(error) => vec![Err(error)],
                };
                futures::stream::iter(items)
            });

        Ok(Box::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ModelRequest {
        ModelRequest {
            model: "auto".into(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: MessageContent::Text("You are a coding agent.".into()),
                },
                Message {
                    role: "user".into(),
                    content: MessageContent::Text("Fix the parser.".into()),
                },
            ],
            temperature: Some(0.2),
            max_tokens: Some(512),
            stream: true,
            tools: Some(vec![Tool {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "read_file".into(),
                    description: Some("Read a project file".into()),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {"path": {"type": "string"}}
                    })),
                },
            }]),
            extra: Default::default(),
        }
    }

    #[test]
    fn anthropic_body_keeps_system_out_of_messages_and_includes_tools() {
        let body = build_anthropic_body(&request(), "claude-sonnet-4-6", true);

        assert_eq!(body["system"], "You are a coding agent.");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["tools"][0]["name"], "read_file");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn anthropic_body_restores_tool_use_and_tool_result_history() {
        let mut request = request();
        request.messages = vec![
            Message {
                role: "assistant".into(),
                content: MessageContent::Text(
                    "[Tool call exec_command id=call_1]\n{\"cmd\":\"pwd\"}".into(),
                ),
            },
            Message {
                role: "user".into(),
                content: MessageContent::Text("[Tool result id=call_1]\n/workspace".into()),
            },
        ];

        let body = build_anthropic_body(&request, "claude-sonnet-4-6", false);

        assert_eq!(body["messages"][0]["content"][0]["type"], "tool_use");
        assert_eq!(body["messages"][0]["content"][0]["id"], "call_1");
        assert_eq!(body["messages"][0]["content"][0]["name"], "exec_command");
        assert_eq!(body["messages"][0]["content"][0]["input"]["cmd"], "pwd");
        assert_eq!(body["messages"][1]["content"][0]["type"], "tool_result");
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "call_1");
    }

    #[test]
    fn anthropic_response_preserves_tool_use_blocks() {
        let parsed = parse_anthropic_response(
            serde_json::json!({
                "id": "msg_1",
                "model": "claude-sonnet-4-6",
                "content": [{
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "exec_command",
                    "input": {"cmd": "pwd"}
                }],
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 10, "output_tokens": 4}
            }),
            "claude-sonnet-4-6",
            25,
        );

        let tool_call = &parsed.choices[0].message.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tool_call.id, "call_1");
        assert_eq!(tool_call.function.name, "exec_command");
        assert_eq!(tool_call.function.arguments, r#"{"cmd":"pwd"}"#);
    }

    #[test]
    fn anthropic_sse_decoder_handles_split_events() {
        let mut decoder = AnthropicSseDecoder::default();

        let first = decoder
            .push(
                br#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","model":"claude-sonnet-4-6","usage":{"input_tokens":12}}}

event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"hel"#,
            )
            .unwrap();
        let second = decoder
            .push(
                br#"lo"}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}

"#,
            )
            .unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].choices[0].delta.role.as_deref(), Some("assistant"));
        assert_eq!(second[0].choices[0].delta.content.as_deref(), Some("hello"));
        assert_eq!(
            second[1].choices[0].finish_reason.as_deref(),
            Some("end_turn")
        );
        assert_eq!(second[1].usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn anthropic_sse_decoder_preserves_tool_argument_deltas() {
        let mut decoder = AnthropicSseDecoder::default();
        let chunks = decoder
            .push(
                br#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","model":"claude-sonnet-4-6","usage":{"input_tokens":12}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call_1","name":"exec_command","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"cmd\":\"pwd\"}"}}

"#,
            )
            .unwrap();

        let tool_chunks = chunks
            .iter()
            .filter_map(|chunk| chunk.choices[0].delta.tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(tool_chunks[0].id, "call_1");
        assert_eq!(tool_chunks[0].function.name, "exec_command");
        assert_eq!(tool_chunks[1].function.arguments, r#"{"cmd":"pwd"}"#);
    }
}
