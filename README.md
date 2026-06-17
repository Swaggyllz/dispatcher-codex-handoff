# Dispatcher 2.0: Codex Handoff Router

Codex-first handoff routing for quota pressure and rate-limit recovery.

[简体中文](README.zh-CN.md) | English

> **Alpha:** Dispatcher 2.0 is a release-candidate continuation of Dispatcher,
> prepared as a new GitHub project. Configuration formats and provider metadata
> may still change before a stable release.

Dispatcher runs an OpenAI-compatible service on your machine. It analyzes each
request, selects a provider and model using quality, cost, latency, capability,
and recent health signals, and records an explainable routing decision. The 2.0
line adds Codex-native emergency handoff packages, quota telemetry, and
user-approved fallback continuation through `provider-auto`.

## Why Dispatcher?

- Codex-native `auto` routing that preserves the Responses request shape
- Emergency `dispatcher_handoff.v1` packages for 429 and quota-pressure events
- Observable quota telemetry without claiming an exact account balance
- User-approved `provider-auto` continuation as degraded fallback execution
- Local dashboard for quota signals, handoffs, routing explanations, usage, and cost
- Built-in demo provider, so you can test the routing surface without an API key

## Quick Start

### 1. Get the source

```bash
git clone https://github.com/Swaggyllz/dispatcher-codex-handoff.git
cd dispatcher-codex-handoff
```

Release archives will be prepared from this new 2.0 repository. Until then,
build from source.

### 2. Start Dispatcher

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run --release -- serve --web-dir ./web/dist
```

Requirements: Rust 1.95 or newer, Node.js 22, and pnpm 10.

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
cargo run --release -- serve --web-dir ./web/dist
```

Never commit real keys. Dispatcher reads credentials from the service process
environment and does not load `.env` automatically.

## Connect Your Coding Agent

### Codex

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

### Claude Code

Claude Code can use Dispatcher as an Anthropic Messages API gateway:

```bash
ANTHROPIC_BASE_URL=http://localhost:8787 \
ANTHROPIC_API_KEY=local-dispatcher \
claude
```

`ANTHROPIC_BASE_URL` uses the service root without `/v1`; Claude Code appends
`/v1/messages` itself. The placeholder client key is accepted locally and is
not forwarded to model providers.

For a persistent user-level setup, add this to `~/.claude/settings.json`:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://localhost:8787",
    "ANTHROPIC_API_KEY": "local-dispatcher"
  }
}
```

Dispatcher supports Anthropic Messages requests, streaming responses, tool
calls, routing, and provider fallback. Broader real-account compatibility
testing is still in progress during the alpha.

See Anthropic's
[LLM gateway documentation](https://docs.anthropic.com/en/docs/claude-code/llm-gateway)
for the upstream Claude Code gateway model.

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

### Codex handoff experiment

Dispatcher 2.0 adds a Codex-first handoff flow for quota pressure. When the
native Codex route observes reliable rate-limit headers, Dispatcher records
quota telemetry; when it observes an emergency 429 or `retry-after`, it also
creates a `dispatcher_handoff.v1` package and shows it in dashboard telemetry.
The dashboard can copy the continuation prompt or, after an explicit user
click, continue through `provider-auto` as degraded execution. This flow does
not promise an exact 10% quota balance, emulate hosted Responses tools, or
switch to a third-party model automatically.

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
