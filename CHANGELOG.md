# Changelog

All notable changes to Dispatcher are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and Semantic Versioning.

## [Unreleased]

## [0.1.0-alpha.1] - 2026-06-15

### Added

- Rust routing engine with task analysis, structured scoring, sticky continuation,
  capability filtering, circuit breaking, and fallback.
- Anthropic, OpenAI, Gemini, OpenRouter, SiliconFlow, DeepSeek, MiMo, Ollama, and
  local Demo providers.
- OpenAI Chat Completions, Anthropic Messages, and OpenAI Responses API endpoints.
- Codex-native `auto` routing and multi-provider `provider-auto` routing.
- OpenAI-compatible `/v1/models` discovery.
- React dashboard for provider health, routing explanations, policy editing, telemetry,
  and cost summaries.
- Configurable per-tier routing policy and provider metadata overrides.

### Verified

- Independent Codex client text, Responses SSE, function-tool round trip, and resumed
  conversation through a real third-party provider.
- Full Rust workspace tests and frontend production build.

### Known Limitations

- `provider-auto` does not emulate OpenAI-hosted tools.
- Provider pricing and capability metadata require ongoing maintenance.
- Release artifacts are unsigned CLI archives.
