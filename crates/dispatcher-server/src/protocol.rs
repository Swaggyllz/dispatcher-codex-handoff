use dispatcher_engine::types::*;

/// Anthropic Messages 请求体
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub system: Option<SystemPrompt>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    Blocks(Vec<SystemBlock>),
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    MultiBlock(Vec<ContentBlock>),
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
}

/// Anthropic → OpenAI 请求转换
pub fn anthropic_to_openai(req: &AnthropicRequest) -> ModelRequest {
    let mut messages: Vec<Message> = Vec::new();

    // Anthropic 的 system 字段 → OpenAI 的 system 消息
    match &req.system {
        Some(SystemPrompt::Text(t)) if !t.is_empty() => {
            messages.push(Message {
                role: "system".into(),
                content: MessageContent::Text(t.clone()),
            });
        }
        Some(SystemPrompt::Blocks(blocks)) => {
            let text: String = blocks
                .iter()
                .filter_map(|b| b.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                messages.push(Message {
                    role: "system".into(),
                    content: MessageContent::Text(text),
                });
            }
        }
        _ => {}
    }

    for m in &req.messages {
        let content = match &m.content {
            AnthropicContent::Text(t) => MessageContent::Text(t.clone()),
            AnthropicContent::MultiBlock(blocks) => {
                let parts: Vec<ContentPart> = blocks
                    .iter()
                    .filter_map(|b| {
                        b.text.as_ref().map(|t| ContentPart {
                            content_type: "text".into(),
                            text: Some(t.clone()),
                            image_url: None,
                        })
                    })
                    .collect();
                if parts.is_empty() {
                    MessageContent::Text(String::new())
                } else {
                    MessageContent::MultiPart(parts)
                }
            }
        };
        messages.push(Message {
            role: m.role.clone(),
            content,
        });
    }

    // 转换 Anthropic tools → OpenAI tools
    let openai_tools: Option<Vec<dispatcher_engine::types::Tool>> =
        req.tools.as_ref().map(|tools| {
            tools
                .iter()
                .map(|t| dispatcher_engine::types::Tool {
                    tool_type: "function".into(),
                    function: dispatcher_engine::types::FunctionDef {
                        name: t["name"].as_str().unwrap_or("").into(),
                        description: t["description"].as_str().map(|s| s.into()),
                        parameters: t.get("input_schema").cloned(),
                    },
                })
                .collect()
        });

    ModelRequest {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_tokens: Some(req.max_tokens),
        stream: req.stream,
        tools: openai_tools,
        extra: Default::default(),
    }
}

/// OpenAI ChatCompletionResponse → Anthropic Messages 响应
pub fn openai_to_anthropic(resp: &ChatCompletionResponse, model: &str) -> serde_json::Value {
    let content_text = resp
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    serde_json::json!({
        "id": resp.id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": content_text}],
        "stop_reason": resp.finish_reason.as_deref().unwrap_or("end_turn"),
        "usage": {
            "input_tokens": resp.usage.prompt_tokens,
            "output_tokens": resp.usage.completion_tokens,
        }
    })
}

/// OpenAI StreamChunk → Anthropic SSE JSON payloads
use std::sync::atomic::{AtomicBool, Ordering};

/// SSE 事件: (event_type, json_payload)
pub struct SsePayload {
    pub event_type: String,
    pub json: String,
}

/// OpenAI StreamChunk → Anthropic SSE 事件列表
pub fn stream_chunk_to_sse_json(
    chunk: &StreamChunk,
    started: &AtomicBool,
    stopped: &AtomicBool,
) -> Vec<SsePayload> {
    let mut events = Vec::new();

    for choice in &chunk.choices {
        let is_first = !started.swap(true, Ordering::Relaxed);

        if is_first {
            let role = choice.delta.role.as_deref().unwrap_or("assistant");
            events.push(SsePayload {
                event_type: "message_start".into(),
                json: serde_json::to_string(&serde_json::json!({
                    "type": "message_start",
                    "message": {"id":chunk.id,"type":"message","role":role,"model":chunk.model,"content":[]}
                })).unwrap_or_default(),
            });
            events.push(SsePayload {
                event_type: "content_block_start".into(),
                json: serde_json::to_string(&serde_json::json!({
                    "type":"content_block_start","index":choice.index,
                    "content_block":{"type":"text","text":""}
                }))
                .unwrap_or_default(),
            });
        }

        if let Some(ref content) = choice.delta.content {
            if !content.is_empty() {
                events.push(SsePayload {
                    event_type: "content_block_delta".into(),
                    json: serde_json::to_string(&serde_json::json!({
                        "type":"content_block_delta","index":choice.index,
                        "delta":{"type":"text_delta","text":content}
                    }))
                    .unwrap_or_default(),
                });
            }
        }

        if choice.finish_reason.is_some() {
            stopped.store(true, Ordering::Relaxed);
            events.push(SsePayload {
                event_type: "content_block_stop".into(),
                json: serde_json::to_string(&serde_json::json!({
                    "type":"content_block_stop","index":choice.index
                }))
                .unwrap_or_default(),
            });
            if let Some(ref usage) = chunk.usage {
                events.push(SsePayload {
                    event_type: "message_delta".into(),
                    json: serde_json::to_string(&serde_json::json!({
                        "type":"message_delta",
                        "delta":{"stop_reason":"end_turn"},
                        "usage":{"output_tokens":usage.completion_tokens}
                    }))
                    .unwrap_or_default(),
                });
            }
            events.push(SsePayload {
                event_type: "message_stop".into(),
                json: serde_json::to_string(&serde_json::json!({
                    "type":"message_stop"
                }))
                .unwrap_or_default(),
            });
        }
    }

    events
}

pub fn stream_end_to_sse_json(started: &AtomicBool, stopped: &AtomicBool) -> Vec<SsePayload> {
    if !started.load(Ordering::Relaxed) || stopped.swap(true, Ordering::Relaxed) {
        return Vec::new();
    }

    vec![
        SsePayload {
            event_type: "content_block_stop".into(),
            json: serde_json::json!({
                "type": "content_block_stop",
                "index": 0
            })
            .to_string(),
        },
        SsePayload {
            event_type: "message_delta".into(),
            json: serde_json::json!({
                "type": "message_delta",
                "delta": {"stop_reason": "end_turn"},
                "usage": {"output_tokens": 0}
            })
            .to_string(),
        },
        SsePayload {
            event_type: "message_stop".into(),
            json: serde_json::json!({"type": "message_stop"}).to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(finish_reason: Option<&str>) -> StreamChunk {
        StreamChunk {
            id: "stream-1".into(),
            model: "test-model".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".into()),
                    content: None,
                    reasoning_content: Some("thinking".into()),
                    tool_calls: None,
                },
                finish_reason: finish_reason.map(str::to_string),
            }],
            usage: None,
        }
    }

    #[test]
    fn stream_end_adds_missing_anthropic_terminal_events() {
        let started = AtomicBool::new(false);
        let stopped = AtomicBool::new(false);

        let initial = stream_chunk_to_sse_json(&chunk(None), &started, &stopped);
        let terminal = stream_end_to_sse_json(&started, &stopped);

        assert_eq!(
            initial
                .iter()
                .map(|payload| payload.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["message_start", "content_block_start"]
        );
        assert_eq!(
            terminal
                .iter()
                .map(|payload| payload.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["content_block_stop", "message_delta", "message_stop"]
        );
    }

    #[test]
    fn stream_end_does_not_duplicate_existing_terminal_events() {
        let started = AtomicBool::new(false);
        let stopped = AtomicBool::new(false);

        let events = stream_chunk_to_sse_json(&chunk(Some("stop")), &started, &stopped);

        assert!(events
            .iter()
            .any(|payload| payload.event_type == "message_stop"));
        assert!(stream_end_to_sse_json(&started, &stopped).is_empty());
    }
}
