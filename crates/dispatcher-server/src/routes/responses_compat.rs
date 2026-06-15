use dispatcher_engine::types::{ChatCompletionResponse, StreamChunk, Usage};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ResponsesSseEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
struct TextOutput {
    output_index: usize,
    item_id: String,
    text: String,
}

#[derive(Debug, Clone)]
struct FunctionOutput {
    output_index: usize,
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
}

pub struct ResponsesStreamState {
    response_id: String,
    model: String,
    sequence_number: u64,
    next_output_index: usize,
    text: Option<TextOutput>,
    functions: BTreeMap<u32, FunctionOutput>,
}

impl ResponsesStreamState {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            response_id: format!("resp_{}", uuid::Uuid::new_v4().simple()),
            model: model.into(),
            sequence_number: 0,
            next_output_index: 0,
            text: None,
            functions: BTreeMap::new(),
        }
    }

    pub fn start(&mut self) -> Vec<ResponsesSseEvent> {
        let response = self.response_snapshot("in_progress", Vec::new(), None);
        vec![
            self.event(
                "response.created",
                serde_json::json!({"response": response}),
            ),
            self.event(
                "response.in_progress",
                serde_json::json!({"response": self.response_snapshot("in_progress", Vec::new(), None)}),
            ),
        ]
    }

    pub fn push(&mut self, chunk: &StreamChunk) -> Vec<ResponsesSseEvent> {
        let mut events = Vec::new();
        for choice in &chunk.choices {
            if let Some(content) = choice
                .delta
                .content
                .as_deref()
                .filter(|text| !text.is_empty())
            {
                if self.text.is_none() {
                    let output = TextOutput {
                        output_index: self.take_output_index(),
                        item_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                        text: String::new(),
                    };
                    events.push(self.event(
                        "response.output_item.added",
                        serde_json::json!({
                            "output_index": output.output_index,
                            "item": message_item(&output, "in_progress"),
                        }),
                    ));
                    events.push(self.event(
                        "response.content_part.added",
                        serde_json::json!({
                            "item_id": output.item_id,
                            "output_index": output.output_index,
                            "content_index": 0,
                            "part": output_text_part(""),
                        }),
                    ));
                    self.text = Some(output);
                }
                let text = self.text.as_mut().expect("text output initialized");
                text.text.push_str(content);
                let item_id = text.item_id.clone();
                let output_index = text.output_index;
                events.push(self.event(
                    "response.output_text.delta",
                    serde_json::json!({
                        "item_id": item_id,
                        "output_index": output_index,
                        "content_index": 0,
                        "delta": content,
                        "logprobs": [],
                    }),
                ));
            }

            for tool_call in choice.delta.tool_calls.as_deref().unwrap_or_default() {
                let index = tool_call.index.unwrap_or(0);
                if !self.functions.contains_key(&index) {
                    let output = FunctionOutput {
                        output_index: self.take_output_index(),
                        item_id: format!("fc_{}", uuid::Uuid::new_v4().simple()),
                        call_id: tool_call.id.clone(),
                        name: String::new(),
                        arguments: String::new(),
                    };
                    events.push(self.event(
                        "response.output_item.added",
                        serde_json::json!({
                            "output_index": output.output_index,
                            "item": function_item(&output, "in_progress"),
                        }),
                    ));
                    self.functions.insert(index, output);
                }

                let output = self
                    .functions
                    .get_mut(&index)
                    .expect("function output initialized");
                if !tool_call.id.is_empty() {
                    output.call_id = tool_call.id.clone();
                }
                if !tool_call.function.name.is_empty() {
                    output.name.push_str(&tool_call.function.name);
                }
                let delta = tool_call.function.arguments.as_str();
                output.arguments.push_str(delta);
                let item_id = output.item_id.clone();
                let output_index = output.output_index;
                if !delta.is_empty() {
                    events.push(self.event(
                        "response.function_call_arguments.delta",
                        serde_json::json!({
                            "item_id": item_id,
                            "output_index": output_index,
                            "delta": delta,
                        }),
                    ));
                }
            }
        }
        events
    }

    pub fn finish(&mut self, usage: Option<Usage>) -> Vec<ResponsesSseEvent> {
        let mut events = Vec::new();
        let mut output = Vec::new();

        if let Some(text) = self.text.clone() {
            events.push(self.event(
                "response.output_text.done",
                serde_json::json!({
                    "item_id": text.item_id,
                    "output_index": text.output_index,
                    "content_index": 0,
                    "text": text.text,
                    "logprobs": [],
                }),
            ));
            events.push(self.event(
                "response.content_part.done",
                serde_json::json!({
                    "item_id": text.item_id,
                    "output_index": text.output_index,
                    "content_index": 0,
                    "part": output_text_part(&text.text),
                }),
            ));
            let item = message_item(&text, "completed");
            events.push(self.event(
                "response.output_item.done",
                serde_json::json!({
                    "output_index": text.output_index,
                    "item": item,
                }),
            ));
            output.push((text.output_index, message_item(&text, "completed")));
        }

        for function in self.functions.values().cloned().collect::<Vec<_>>() {
            events.push(self.event(
                "response.function_call_arguments.done",
                serde_json::json!({
                    "item_id": function.item_id,
                    "output_index": function.output_index,
                    "arguments": function.arguments,
                }),
            ));
            let item = function_item(&function, "completed");
            events.push(self.event(
                "response.output_item.done",
                serde_json::json!({
                    "output_index": function.output_index,
                    "item": item,
                }),
            ));
            output.push((function.output_index, function_item(&function, "completed")));
        }

        output.sort_by_key(|(index, _)| *index);
        let output = output.into_iter().map(|(_, item)| item).collect();
        let completed = self.response_snapshot("completed", output, usage);
        events.push(self.event(
            "response.completed",
            serde_json::json!({"response": completed}),
        ));
        events
    }

    pub fn fail(&mut self, message: &str) -> ResponsesSseEvent {
        let mut response = self.response_snapshot("failed", Vec::new(), None);
        response["error"] = serde_json::json!({
            "code": "provider_error",
            "message": message,
        });
        self.event("response.failed", serde_json::json!({"response": response}))
    }

    fn take_output_index(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        index
    }

    fn event(&mut self, event_type: &str, mut data: serde_json::Value) -> ResponsesSseEvent {
        if let Some(object) = data.as_object_mut() {
            object.insert("type".into(), serde_json::json!(event_type));
            object.insert(
                "sequence_number".into(),
                serde_json::json!(self.sequence_number),
            );
        }
        self.sequence_number += 1;
        ResponsesSseEvent {
            event_type: event_type.into(),
            data,
        }
    }

    fn response_snapshot(
        &self,
        status: &str,
        output: Vec<serde_json::Value>,
        usage: Option<Usage>,
    ) -> serde_json::Value {
        let usage = usage.unwrap_or(Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });
        serde_json::json!({
            "id": self.response_id,
            "object": "response",
            "created_at": chrono::Utc::now().timestamp(),
            "status": status,
            "error": null,
            "incomplete_details": null,
            "model": self.model,
            "output": output,
            "parallel_tool_calls": true,
            "usage": {
                "input_tokens": usage.prompt_tokens,
                "input_tokens_details": {"cached_tokens": 0},
                "output_tokens": usage.completion_tokens,
                "output_tokens_details": {"reasoning_tokens": 0},
                "total_tokens": usage.total_tokens,
            },
        })
    }
}

fn output_text_part(text: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "output_text",
        "text": text,
        "annotations": [],
        "logprobs": [],
    })
}

fn message_item(output: &TextOutput, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": output.item_id,
        "type": "message",
        "status": status,
        "role": "assistant",
        "content": [output_text_part(&output.text)],
    })
}

fn function_item(output: &FunctionOutput, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": output.item_id,
        "type": "function_call",
        "status": status,
        "call_id": output.call_id,
        "name": output.name,
        "arguments": output.arguments,
    })
}

pub fn chat_completion_to_response(response: &ChatCompletionResponse) -> serde_json::Value {
    let choice = response.choices.first();
    let mut output = Vec::new();

    if let Some(tool_calls) = choice.and_then(|choice| choice.message.tool_calls.as_ref()) {
        output.extend(tool_calls.iter().map(|tool_call| {
            serde_json::json!({
                "id": format!("fc_{}", uuid::Uuid::new_v4().simple()),
                "type": "function_call",
                "status": "completed",
                "call_id": tool_call.id,
                "name": tool_call.function.name,
                "arguments": tool_call.function.arguments,
            })
        }));
    }

    let text = choice
        .map(|choice| choice.message.content.as_str())
        .unwrap_or_default();
    if !text.is_empty() {
        output.push(serde_json::json!({
            "id": format!("msg_{}", uuid::Uuid::new_v4().simple()),
            "type": "message",
            "status": "completed",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text,
                "annotations": [],
                "logprobs": [],
            }],
        }));
    }

    serde_json::json!({
        "id": format!("resp_{}", uuid::Uuid::new_v4().simple()),
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "status": "completed",
        "error": null,
        "incomplete_details": null,
        "model": response.model,
        "output": output,
        "parallel_tool_calls": true,
        "usage": {
            "input_tokens": response.usage.prompt_tokens,
            "input_tokens_details": {"cached_tokens": 0},
            "output_tokens": response.usage.completion_tokens,
            "output_tokens_details": {"reasoning_tokens": 0},
            "total_tokens": response.usage.total_tokens,
        },
    })
}

#[cfg(test)]
mod tests {
    use dispatcher_engine::types::{
        ChatCompletionResponse, Choice, FunctionCall, ResponseMessage, StreamChoice, StreamChunk,
        StreamDelta, ToolCall, Usage,
    };

    use super::{chat_completion_to_response, ResponsesStreamState};

    fn response(content: &str, tool_calls: Option<Vec<ToolCall>>) -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "chatcmpl_1".into(),
            model: "test-model".into(),
            provider: "test-provider".into(),
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: content.into(),
                    tool_calls,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: Some("stop".into()),
            latency_ms: 12,
        }
    }

    #[test]
    fn non_stream_text_becomes_responses_message_output() {
        let value = chat_completion_to_response(&response("hello", None));

        assert_eq!(value["object"], "response");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["model"], "test-model");
        assert_eq!(value["output"][0]["type"], "message");
        assert_eq!(value["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(value["output"][0]["content"][0]["text"], "hello");
        assert_eq!(value["usage"]["input_tokens"], 10);
        assert_eq!(value["usage"]["output_tokens"], 5);
    }

    #[test]
    fn non_stream_tool_call_becomes_responses_function_call_output() {
        let value = chat_completion_to_response(&response(
            "",
            Some(vec![ToolCall {
                index: Some(0),
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "exec_command".into(),
                    arguments: r#"{"cmd":"pwd"}"#.into(),
                },
            }]),
        ));

        assert_eq!(value["output"][0]["type"], "function_call");
        assert_eq!(value["output"][0]["call_id"], "call_1");
        assert_eq!(value["output"][0]["name"], "exec_command");
        assert_eq!(value["output"][0]["arguments"], r#"{"cmd":"pwd"}"#);
        assert_eq!(value["output"][0]["status"], "completed");
    }

    #[test]
    fn stream_text_emits_responses_lifecycle_events() {
        let mut state = ResponsesStreamState::new("test-model");
        let mut events = state.start();
        events.extend(state.push(&StreamChunk {
            id: "chunk_1".into(),
            model: "test-model".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".into()),
                    content: Some("hello".into()),
                    reasoning_content: None,
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: None,
        }));
        events.extend(state.finish(Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        })));

        let names = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        assert_eq!(events[4].data["delta"], "hello");
        assert_eq!(
            events.last().unwrap().data["response"]["usage"]["total_tokens"],
            15
        );
    }

    #[test]
    fn stream_tool_call_merges_argument_deltas_and_completes_item() {
        let mut state = ResponsesStreamState::new("test-model");
        let mut events = state.start();

        for (name, arguments) in [("exec_command", "{\"cmd\":\""), ("", r#"pwd"}"#)] {
            events.extend(state.push(&StreamChunk {
                id: "chunk_1".into(),
                model: "test-model".into(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: Some(vec![ToolCall {
                            index: Some(0),
                            id: "call_1".into(),
                            call_type: "function".into(),
                            function: FunctionCall {
                                name: name.into(),
                                arguments: arguments.into(),
                            },
                        }]),
                    },
                    finish_reason: None,
                }],
                usage: None,
            }));
        }
        events.extend(state.finish(None));

        assert!(events
            .iter()
            .any(|event| event.event_type == "response.function_call_arguments.delta"));
        let done = events
            .iter()
            .find(|event| event.event_type == "response.function_call_arguments.done")
            .unwrap();
        assert_eq!(done.data["arguments"], r#"{"cmd":"pwd"}"#);
        let item_done = events
            .iter()
            .find(|event| {
                event.event_type == "response.output_item.done"
                    && event.data["item"]["type"] == "function_call"
            })
            .unwrap();
        assert_eq!(item_done.data["item"]["call_id"], "call_1");
        assert_eq!(item_done.data["item"]["name"], "exec_command");
    }
}
