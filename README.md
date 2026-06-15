# Dispatcher

Local intelligent model routing for AI coding agents.

[简体中文](README.zh-CN.md) | English

> **Alpha:** `v0.1.0-alpha.1` is ready for local evaluation. Configuration
> formats and provider metadata may still change before `v1.0`.

Dispatcher runs an OpenAI-compatible service on your machine. It analyzes each
request, selects a provider and model using quality, cost, latency, capability,
and recent health signals, and records an explainable routing decision.

## Why Dispatcher?

- One local endpoint for Codex, OpenAI-compatible clients, and Anthropic clients
- Automatic model selection across `simple`, `medium`, `reasoning`, and `complex` tasks
- `auto`, `save`, and `fast` routing strategies
- Provider health scoring, circuit breaking, timeout protection, and fallback
- Tool, vision, streaming, and context-window capability filtering
- Local dashboard for routing explanations, usage, latency, and cost
- Built-in demo provider, so you can try the complete flow without an API key

## Quick Start

### 1. Download

Download the archive for your system from
[v0.1.0-alpha.1](https://github.com/Swaggyllz/dispatcher/releases/tag/v0.1.0-alpha.1):

| Platform            | Package                           |
| ------------------- | --------------------------------- |
| macOS Apple Silicon | `dispatcher-macos-aarch64.tar.gz` |
| Linux x86_64        | `dispatcher-linux-x86_64.tar.gz`  |
| Windows x86_64      | `dispatcher-windows-x86_64.zip`   |

The binaries are currently unsigned. Your operating system may ask you to
confirm that you trust the downloaded file.

### 2. Start Dispatcher

macOS or Linux:

```bash
tar -xzf dispatcher-*.tar.gz
./dispatch serve --web-dir ./web/dist
```

Windows PowerShell:

```powershell
Expand-Archive .\dispatcher-windows-x86_64.zip -DestinationPath .\dispatcher
cd .\dispatcher
.\dispatch.exe serve --web-dir .\web\dist
```

### 3. Open the dashboard

Open [http://localhost:8787](http://localhost:8787). The API is available at
`http://localhost:8787/v1`.

No provider key is required for the first run. The demo provider lets you test
routing from the dashboard immediately.

### 4. Add a real provider

Set one or more provider keys before starting Dispatcher:

```bash
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
./dispatch serve --web-dir ./web/dist
```

Never commit real keys. Dispatcher reads credentials from the service process
environment and does not load `.env` automatically.

## Connect Codex

Add this profile to `~/.codex/config.toml`:

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

This keeps requests on the Codex-native model lane while Dispatcher selects the
model, reasoning effort, and speed. See
[Codex routing modes](#codex-routing-modes) for multi-provider routing.

## Supported Providers

| Provider    | Environment variable                                  |
| ----------- | ----------------------------------------------------- |
| Anthropic   | `ANTHROPIC_API_KEY`                                   |
| OpenAI      | `OPENAI_API_KEY`                                      |
| Gemini      | `GEMINI_API_KEY`                                      |
| OpenRouter  | `OPENROUTER_API_KEY`                                  |
| SiliconFlow | `SILICONFLOW_API_KEY`                                 |
| DeepSeek    | `DEEPSEEK_API_KEY`                                    |
| Xiaomi MiMo | `MIMO_API_KEY` or `XIAOMIMIMO_API_KEY`                |
| Ollama      | `OLLAMA_BASE_URL` (default: `http://localhost:11434`) |

See [`.env.example`](.env.example) for the complete environment variable list.

## How It Works

```text
Client request
    |
    v
Protocol compatibility layer
    |
    v
Task analysis and capability filtering
    |
    v
Quality / cost / latency / health scoring
    |
    v
Provider execution, timeout, and fallback
    |
    v
Compatible response + local telemetry
```

Dispatcher classifies the task, removes models that cannot satisfy required
capabilities, scores the remaining candidates, and applies health and circuit
breaker state before execution. Routing decisions remain observable through
the dashboard and telemetry API.

## API

| Endpoint                    | Purpose                           |
| --------------------------- | --------------------------------- |
| `GET /v1/health`            | Service health                    |
| `GET /v1/models`            | OpenAI-compatible model discovery |
| `GET /v1/providers`         | Provider capabilities and health  |
| `GET /v1/telemetry`         | Usage and cost summaries          |
| `POST /v1/chat/completions` | OpenAI-compatible chat            |
| `POST /v1/messages`         | Anthropic-compatible messages     |
| `POST /v1/responses`        | Codex/OpenAI Responses API        |

Example:

```bash
curl http://localhost:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Explain this repository"}]
  }'
```

## Codex Routing Modes

### Native Codex routing

Use `X-Dispatcher-Mode = "auto"` to stay on the Codex-native lane. Dispatcher
chooses model, reasoning effort, and speed without converting the request to a
third-party provider protocol.

### Multi-provider routing

Use `provider-auto` to route Responses requests using provider credentials
configured in Dispatcher:

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

OpenAI-hosted tools are not emulated in this mode. The Codex bearer token and
`ChatGPT-Account-Id` are not forwarded to third-party providers.

## Configuration

Start with the included example:

```bash
./dispatch serve \
  --web-dir ./web/dist \
  --config ./dispatcher.example.toml
```

The file defines routing strategies and per-tier overrides. Provider model
metadata can be replaced with:

```bash
export DISPATCHER_PROVIDER_METADATA=/path/to/provider-models.toml
```

The service binds to `127.0.0.1` by default. Set `DISPATCHER_BIND_ADDR` only
when another interface is required and protected by suitable authentication
and network controls.

## Build From Source

Requirements:

- Rust 1.95 or newer
- Node.js 22
- pnpm 10

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run --release -- serve --web-dir ./web/dist
```

## Development

Run the same checks used by CI:

```bash
./scripts/check-open-source-readiness.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
```

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before
opening a pull request. Report vulnerabilities privately according to
[SECURITY.md](SECURITY.md).

## Alpha Limitations

- Release binaries are unsigned.
- Provider prices and capabilities change frequently; bundled metadata is not a billing guarantee.
- Anthropic-native tool conversion needs broader real-account testing.
- Hosted Responses tools such as web search and image generation are unavailable in `provider-auto`.
- The release is a CLI plus static dashboard, not a signed desktop application.
- Multi-user authentication and tenant isolation are not implemented.

Keep the default loopback binding unless you have secure network controls.

## Documentation

- [Chinese user manual](docs/mvp-user-manual-zh.md)
- [Routing research (Chinese)](docs/routing-research-and-product-answer-zh.md)
- [Changelog](CHANGELOG.md)
- [Support](SUPPORT.md)

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution.
