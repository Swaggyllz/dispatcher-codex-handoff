# Dispatcher

Local intelligent model routing for AI coding agents.

> Status: `v0.1.0-alpha`. The core routing path is usable and tested, but configuration
> formats and provider metadata may still change before `v1.0`.

Dispatcher runs an OpenAI-compatible HTTP service on your machine. It analyzes each
request, selects a provider and model using quality, cost, latency, capability, and
recent health signals, then records an explainable routing decision and usage telemetry.

中文说明见 [MVP 使用手册](docs/mvp-user-manual-zh.md)。

## Highlights

- Codex Responses API support with native `auto` and multi-provider `provider-auto` modes
- OpenAI Chat Completions and Anthropic Messages compatible entry points
- Task tiers: `simple`, `medium`, `reasoning`, and `complex`
- `auto`, `save`, and `fast` routing strategies
- Tool, vision, streaming, and context-window capability filtering
- Provider health scoring, circuit breaking, timeout protection, and fallback
- Local React dashboard with routing explanations and cost telemetry
- Demo provider enabled by default, so the full local flow works without an API key

## Supported Providers

Dispatcher registers providers from environment variables:

| Provider | Environment variable |
| --- | --- |
| Anthropic | `ANTHROPIC_API_KEY` |
| OpenAI | `OPENAI_API_KEY` |
| Gemini | `GEMINI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |
| SiliconFlow | `SILICONFLOW_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| Xiaomi MiMo | `MIMO_API_KEY` or `XIAOMIMIMO_API_KEY` |
| Ollama | `OLLAMA_BASE_URL` (defaults to `http://localhost:11434`) |

Copy [`.env.example`](.env.example) for the complete list. Dispatcher does not load
`.env` automatically; export the variables in the service process.

The service listens on `127.0.0.1` by default. Set `DISPATCHER_BIND_ADDR` explicitly
only when another interface is required.

## Quick Start

Requirements:

- Rust 1.95 or newer
- Node.js 22
- pnpm 10

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run --release -- serve --web-dir ./web/dist
```

Open [http://localhost:8787](http://localhost:8787). The API is available under
`http://localhost:8787/v1`.

Useful endpoints:

| Endpoint | Purpose |
| --- | --- |
| `GET /v1/health` | Service health |
| `GET /v1/models` | OpenAI-compatible model discovery |
| `GET /v1/providers` | Provider capabilities and health |
| `GET /v1/telemetry` | Usage and cost summaries |
| `POST /v1/chat/completions` | OpenAI-compatible chat |
| `POST /v1/messages` | Anthropic-compatible messages |
| `POST /v1/responses` | Codex/OpenAI Responses API |

## Codex Configuration

### Native Codex routing

This mode keeps requests on the Codex-native model lane and chooses model, reasoning
effort, and speed:

```toml
model = "gpt-5.5"
model_provider = "dispatcher"

[model_providers.dispatcher]
name = "Dispatcher"
base_url = "http://localhost:8787/v1"
wire_api = "responses"
requires_openai_auth = true
http_headers = { "X-Dispatcher-Mode" = "auto" }
```

### Multi-provider routing

This mode converts Responses requests into Dispatcher provider requests. OpenAI-hosted
tools are not emulated, so disable them in this profile:

```toml
model = "gpt-5.5"
model_provider = "dispatcher"
web_search = "disabled"

[features]
image_generation = false

[model_providers.dispatcher]
name = "Dispatcher Multi-provider"
base_url = "http://localhost:8787/v1"
wire_api = "responses"
requires_openai_auth = true
http_headers = { "X-Dispatcher-Mode" = "provider-auto" }
```

`provider-auto` uses only provider credentials configured in the Dispatcher process.
The Codex client bearer token and `ChatGPT-Account-Id` are not forwarded to third-party
providers.

## Configuration

Routing policy can be loaded from an explicit file:

```bash
cargo run --release -- serve \
  --web-dir ./web/dist \
  --config ./dispatcher.example.toml
```

See [dispatcher.example.toml](dispatcher.example.toml) for strategy and per-tier
overrides. Provider model metadata can be overridden with
`DISPATCHER_PROVIDER_METADATA=/path/to/provider-models.toml`.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
```

The repository contains no production credentials. Runtime databases, `.env` files,
local routing configuration, and build outputs are ignored by Git.

## Alpha Limitations

- Anthropic-native tool conversion has unit coverage but still needs broader real-account
  testing.
- Provider prices and model capabilities change frequently; bundled metadata is a
  starting point, not a billing guarantee.
- Hosted Responses tools such as web search and image generation are not available in
  `provider-auto`.
- The current release artifact is a CLI plus static dashboard, not a signed desktop app.
- The HTTP service does not provide multi-user authentication or tenant isolation. Keep
  the default loopback binding unless secure network controls are in place.

## Documentation

- [MVP user manual (Chinese)](docs/mvp-user-manual-zh.md)
- [Routing research (Chinese)](docs/routing-research-and-product-answer-zh.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution.
