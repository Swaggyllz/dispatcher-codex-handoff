# Domestic Model Compatibility

## Existing Providers

Dispatcher currently includes these domestic or alternate fallback candidates:

- DeepSeek
- SiliconFlow
- Xiaomi MiMo
- OpenRouter
- Ollama

## Tool And Streaming Capability

| Provider | Streaming | Tools | MVP Role |
| --- | --- | --- | --- |
| DeepSeek | Yes | Yes | First domestic agentic fallback |
| SiliconFlow | Yes | Provider-level yes | Candidate after model-level validation |
| Xiaomi MiMo | Yes | No | Text-only continuation |
| OpenRouter | Yes | Yes | Aggregator fallback with allowlist |
| Ollama | No in current code | No | Local text-only/non-agentic fallback later |

## Key Risk

Codex continuation is not just text generation. It needs reliable function calling and tool-result history handling.

Current routing can filter providers by declared capability, but capability is metadata-driven. If a provider declares tools at provider level while a specific model does not reliably support tools, Codex tool execution can fail at runtime.

This matters most for:

- SiliconFlow, because it aggregates many model families.
- OpenRouter, because provider/model behavior varies widely.

## MVP Default Fallback Chain

Recommended MVP chain for tool-using Codex tasks:

```text
DeepSeek -> SiliconFlow allowlisted model -> OpenRouter allowlisted model
```

Recommended text-only chain:

```text
DeepSeek -> SiliconFlow -> MiMo
```

Do not use MiMo or Ollama for MVP agentic tool execution unless their capabilities change and are verified.

## Validation Matrix

Before enabling a model as default agentic fallback, test:

- non-stream function call
- stream function-call arguments
- function-call output continuation
- multi-step tool loop
- malformed tool argument recovery
- refusal to use unsupported hosted tools
- long prompt with handoff package
- fallback after a 429 event

## Product Wording

Use:

```text
Fallback execution mode
```

Avoid:

```text
Same quality as Codex native
```

The fallback model is a continuation worker, not a hidden replacement for Codex.

