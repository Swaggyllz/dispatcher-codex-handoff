# Codex Protocol Research

## Current Modes

Dispatcher already supports three Codex-relevant modes.

### Native Auto

`/v1/responses` with `X-Dispatcher-Mode: auto`, or model `dispatcher-auto`, keeps the native Codex/OpenAI Responses path. Dispatcher analyzes the request and rewrites:

- model
- `reasoning.effort`
- `service_tier`

It then forwards the request upstream. This path preserves native Responses behavior, unknown fields, hosted tools, function tools, reasoning summaries, and native stream events.

Key files:

- `crates/dispatcher-server/src/routes/responses.rs`
- `docs/mvp-user-manual-zh.md`
- `README.zh-CN.md`

### Locked Native Mode

When not in dispatcher-auto and the requested model is one of the known Codex models, Dispatcher preserves the user's explicit model choice and only validates supplied reasoning effort where applicable.

### Provider Auto

`/v1/responses` with `X-Dispatcher-Mode: provider-auto` converts Responses API input into Dispatcher internal `ModelRequest`, routes across configured providers, and converts OpenAI-compatible chat output back into Responses JSON or SSE.

This is the bridge that makes Codex able to indirectly use domestic or alternate models.

## What Already Supports Domestic Providers

Existing provider support includes:

- DeepSeek via `DEEPSEEK_API_KEY`
- SiliconFlow via `SILICONFLOW_API_KEY`
- Xiaomi MiMo via `MIMO_API_KEY` or `XIAOMIMIMO_API_KEY`
- OpenRouter via `OPENROUTER_API_KEY`
- Ollama via `OLLAMA_BASE_URL`

DeepSeek and SiliconFlow use OpenAI-compatible chat completions and support streaming and function tools according to current implementation. MiMo supports streaming but not tools. Ollama currently does not support streaming or tools in this project.

## Provider-Auto Limits

Provider-auto is useful but lossy:

- Hosted Responses tools are not supported.
- Web search and image generation must be disabled.
- Only standard `function` tools are converted into internal tool definitions.
- `custom` and `tool_search` definitions are tolerated but ignored by conversion.
- Unknown Responses input item types are ignored.
- Streaming Responses events are synthetic and built from OpenAI-compatible chat chunks.
- Reasoning summaries are not converted; usage reports `reasoning_tokens: 0`.
- Codex client bearer token and `ChatGPT-Account-Id` are not forwarded to third-party providers.

## 2.0 Protocol Boundary

Dispatcher 2.0 should use two explicit protocol contracts.

### Native Codex Contract

Use:

```text
/v1/responses
X-Dispatcher-Mode: auto
```

This path is transparent transport. Dispatcher may route model, effort, speed, add diagnostic headers, and record telemetry. It should not reinterpret native tools or stream events.

### Provider Handoff Contract

Use:

```text
/v1/responses
X-Dispatcher-Mode: provider-auto
```

This path is a lossy compatibility bridge for:

- text
- supported image input
- function tools
- function-call history and results
- streaming text
- streaming function-call arguments
- provider fallback
- routing headers

It must explicitly reject hosted tools and avoid promising native Responses parity.

## MVP Recommendation

Make the handoff boundary a compact internal representation centered on `ModelRequest`:

- messages
- multipart text/images
- function tool specs
- function call/result history
- stream flag
- routing and session metadata

Do not carry native Responses fields into the fallback path unless they can be mapped losslessly.

