use dispatcher_engine::types::*;
use futures::StreamExt;
use reqwest::Client;

pub(crate) fn build_openai_compat_body(
    request: &ModelRequest,
    model_id: &str,
    stream: bool,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model_id,
        "messages": build_openai_messages(request),
        "temperature": request.temperature.unwrap_or(0.7),
        "max_tokens": request.max_tokens.unwrap_or(4096),
        "stream": stream,
    });
    if let Some(tools) = request.tools.as_ref().filter(|tools| !tools.is_empty()) {
        body["tools"] = serde_json::to_value(tools).unwrap_or_default();
    }
    body
}

fn build_openai_messages(request: &ModelRequest) -> Vec<serde_json::Value> {
    request
        .messages
        .iter()
        .map(|message| {
            if let MessageContent::Text(text) = &message.content {
                if message.role == "assistant" {
                    if let Some((name, call_id, arguments)) = parse_tool_call_marker(text) {
                        return serde_json::json!({
                            "role": "assistant",
                            "content": null,
                            "tool_calls": [{
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments,
                                }
                            }]
                        });
                    }
                }
                if message.role == "user" {
                    if let Some((call_id, output)) = parse_tool_result_marker(text) {
                        return serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call_id,
                            "content": output,
                        });
                    }
                }
            }

            let content = match &message.content {
                MessageContent::Text(text) => serde_json::Value::String(text.clone()),
                MessageContent::MultiPart(parts) => serde_json::json!(parts),
            };
            serde_json::json!({"role": message.role, "content": content})
        })
        .collect()
}

pub(crate) fn parse_tool_call_marker(text: &str) -> Option<(&str, &str, &str)> {
    let rest = text.strip_prefix("[Tool call ")?;
    let (name, rest) = rest.split_once(" id=")?;
    let (call_id, arguments) = rest.split_once("]\n")?;
    Some((name, call_id, arguments))
}

pub(crate) fn parse_tool_result_marker(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[Tool result id=")?;
    rest.split_once("]\n")
}

pub(crate) fn parse_openai_compat_response(
    json: serde_json::Value,
    provider: &str,
    fallback_model: &str,
    latency_ms: u64,
) -> ChatCompletionResponse {
    let choice = &json["choices"][0];
    let usage = &json["usage"];
    let tool_calls = choice["message"]["tool_calls"]
        .as_array()
        .filter(|calls| !calls.is_empty())
        .and_then(|calls| serde_json::from_value::<Vec<ToolCall>>(calls.clone().into()).ok());

    ChatCompletionResponse {
        id: json["id"].as_str().unwrap_or("").into(),
        model: json["model"].as_str().unwrap_or(fallback_model).into(),
        provider: provider.into(),
        choices: vec![Choice {
            index: choice["index"].as_u64().unwrap_or(0) as u32,
            message: ResponseMessage {
                role: choice["message"]["role"]
                    .as_str()
                    .unwrap_or("assistant")
                    .into(),
                content: choice["message"]["content"].as_str().unwrap_or("").into(),
                tool_calls,
            },
            finish_reason: choice["finish_reason"].as_str().map(str::to_string),
        }],
        usage: Usage {
            prompt_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage["total_tokens"].as_u64().unwrap_or(0) as u32,
        },
        finish_reason: choice["finish_reason"].as_str().map(str::to_string),
        latency_ms,
    }
}

#[derive(Default)]
struct OpenAiSseDecoder {
    buffer: String,
}

impl OpenAiSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Vec<Result<StreamChunk, ProviderError>> {
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
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            chunks.push(
                serde_json::from_str::<StreamChunk>(&data)
                    .map_err(|error| ProviderError::Other(error.to_string())),
            );
        }

        chunks
    }
}

/// 通用的 OpenAI 兼容 SSE 流式请求
pub async fn stream_openai_compat(
    client: &Client,
    url: &str,
    api_key: &str,
    model_id: &str,
    request: &ModelRequest,
) -> Result<
    Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send + Unpin>,
    ProviderError,
> {
    let body = build_openai_compat_body(request, model_id, true);

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let status = resp.status();
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

    let stream = resp
        .bytes_stream()
        .scan(OpenAiSseDecoder::default(), |decoder, result| {
            let items = match result {
                Ok(bytes) => decoder.push(&bytes),
                Err(error) => vec![Err(ProviderError::Network(error.to_string()))],
            };
            std::future::ready(Some(items))
        })
        .flat_map(futures::stream::iter);

    Ok(Box::new(stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ModelRequest {
        ModelRequest {
            model: "auto".into(),
            messages: vec![Message {
                role: "user".into(),
                content: MessageContent::Text("hello".into()),
            }],
            temperature: None,
            max_tokens: Some(32),
            stream: true,
            tools: Some(vec![Tool {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "read_file".into(),
                    description: Some("Read a file".into()),
                    parameters: Some(serde_json::json!({"type":"object"})),
                },
            }]),
            extra: Default::default(),
        }
    }

    #[test]
    fn openai_compat_body_forwards_tools() {
        let body = build_openai_compat_body(&request(), "test-model", true);

        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "read_file");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn openai_compat_body_restores_responses_tool_history() {
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

        let body = build_openai_compat_body(&request, "test-model", false);

        assert_eq!(body["messages"][0]["role"], "assistant");
        assert_eq!(body["messages"][0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            body["messages"][0]["tool_calls"][0]["function"]["name"],
            "exec_command"
        );
        assert_eq!(body["messages"][1]["role"], "tool");
        assert_eq!(body["messages"][1]["tool_call_id"], "call_1");
        assert_eq!(body["messages"][1]["content"], "/workspace");
    }

    #[test]
    fn openai_compat_response_preserves_tool_calls() {
        let parsed = parse_openai_compat_response(
            serde_json::json!({
                "id": "chatcmpl_1",
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "exec_command",
                                "arguments": "{\"cmd\":\"pwd\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 4,
                    "total_tokens": 14
                }
            }),
            "test-provider",
            "fallback-model",
            25,
        );

        let tool_call = &parsed.choices[0].message.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tool_call.id, "call_1");
        assert_eq!(tool_call.function.name, "exec_command");
        assert_eq!(tool_call.function.arguments, "{\"cmd\":\"pwd\"}");
    }

    #[test]
    fn openai_sse_decoder_keeps_split_events_and_all_events_in_a_chunk() {
        let mut decoder = OpenAiSseDecoder::default();
        let first = decoder.push(
            br#"data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":"assistant","content":"hel"},"finish_reason":null}],"usage":null}

data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":null,"content":"lo"},"finish_reason":null}],"usage":nu"#,
        );
        let second = decoder.push(
            br#"ll}

data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":null,"content":null},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}

data: [DONE]

"#,
        );

        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].as_ref().unwrap().choices[0]
                .delta
                .content
                .as_deref(),
            Some("hel")
        );
        assert_eq!(second.len(), 2);
        assert_eq!(
            second[0].as_ref().unwrap().choices[0]
                .delta
                .content
                .as_deref(),
            Some("lo")
        );
        assert_eq!(
            second[1].as_ref().unwrap().choices[0]
                .finish_reason
                .as_deref(),
            Some("stop")
        );
    }

    #[test]
    fn openai_sse_decoder_preserves_tool_call_deltas() {
        let mut decoder = OpenAiSseDecoder::default();
        let chunks = decoder.push(
            br#"data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"exec_command","arguments":"{\"cmd\":\""}}]},"finish_reason":null}],"usage":null}

data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":null,"content":null,"tool_calls":[{"index":0,"id":"","type":"function","function":{"name":"","arguments":"pwd\"}"}}]},"finish_reason":null}],"usage":null}

"#,
        );

        let tool_calls = chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .filter_map(|chunk| chunk.choices[0].delta.tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].function.name, "exec_command");
        assert_eq!(tool_calls[0].function.arguments, "{\"cmd\":\"");
        assert_eq!(tool_calls[1].function.arguments, "pwd\"}");
    }

    #[test]
    fn openai_sse_decoder_accepts_null_tool_call_delta_fields() {
        let mut decoder = OpenAiSseDecoder::default();
        let chunks = decoder.push(
            br#"data: {"id":"1","model":"test","choices":[{"index":0,"delta":{"role":null,"content":null,"tool_calls":[{"index":0,"id":null,"type":null,"function":{"name":null,"arguments":"pwd\"}"}}]},"finish_reason":null}],"usage":null}

"#,
        );

        assert!(chunks[0].is_ok(), "{:?}", chunks[0]);
        let tool_call = &chunks[0].as_ref().unwrap().choices[0]
            .delta
            .tool_calls
            .as_ref()
            .unwrap()[0];
        assert_eq!(tool_call.id, "");
        assert_eq!(tool_call.call_type, "");
        assert_eq!(tool_call.function.name, "");
        assert_eq!(tool_call.function.arguments, "pwd\"}");
    }
}
