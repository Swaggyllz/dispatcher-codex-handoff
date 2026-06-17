# Dispatcher 2.0：Codex Handoff Router

面向 Codex 额度压力和限流恢复的本地交接路由器。

简体中文 | [English](README.md)

> **Alpha 版本：** Dispatcher 2.0 是 Dispatcher 的延续版本，准备作为新的 GitHub
> 项目发布。稳定版之前，配置格式和模型供应商元数据仍可能调整。

Dispatcher 在你的电脑上运行一个兼容 OpenAI API 的服务。它会分析每个请求，
结合质量、成本、延迟、能力和近期健康状态选择供应商与模型，并记录可解释的
路由决策。2.0 主线新增 Codex 原生应急交接包、额度遥测，以及用户明确批准后通过
`provider-auto` 进行的降级续接。

## 为什么使用 Dispatcher？

- Codex 原生 `auto` 路由，保留 Responses 请求形态
- 在 429 或额度压力事件中生成 `dispatcher_handoff.v1` 应急交接包
- 记录可观测 quota telemetry，但不承诺精确账户余额
- 用户明确批准后，通过 `provider-auto` 以降级执行模式续接
- 本地控制台展示 quota signal、交接包、路由解释、用量和成本
- 默认内置演示供应商，无需 API Key 也能测试路由入口

## 5 分钟快速上手

### 1. 获取源码

```bash
git clone https://github.com/Swaggyllz/dispatcher-codex-handoff.git
cd dispatcher-codex-handoff
```

正式发布包会从这个新的 2.0 仓库生成。在发布包准备好之前，请先从源码构建。

### 2. 启动 Dispatcher

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run --release -- serve --web-dir ./web/dist
```

环境要求：Rust 1.95 或更高版本、Node.js 22、pnpm 10。

### 3. 打开控制台

访问 [http://localhost:8787](http://localhost:8787)。API 地址为
`http://localhost:8787/v1`。

首次启动不需要配置模型供应商密钥，可以直接在控制台中使用演示供应商测试路由。

### 4. 接入真实模型供应商

启动前设置一个或多个供应商密钥：

```bash
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
cargo run --release -- serve --web-dir ./web/dist
```

不要把真实密钥提交到 Git。Dispatcher 从服务进程的环境变量读取凭据，
不会自动加载 `.env` 文件。

## 接入编程智能体

### Codex

在 `~/.codex/config.toml` 中加入：

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

该模式保留 Codex 原生模型链路，由 Dispatcher 选择模型、推理强度和速度。
需要跨供应商路由时，请参阅后文的 [Codex 路由模式](#codex-路由模式)。

### Claude Code

Claude Code 可以把 Dispatcher 作为 Anthropic Messages API 网关：

```bash
ANTHROPIC_BASE_URL=http://localhost:8787 \
ANTHROPIC_API_KEY=local-dispatcher \
claude
```

`ANTHROPIC_BASE_URL` 使用服务根地址，不要添加 `/v1`；Claude Code 会自行拼接
`/v1/messages`。这里的客户端占位密钥只在本机使用，不会转发给模型供应商。

需要长期生效时，可以写入用户级 `~/.claude/settings.json`：

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://localhost:8787",
    "ANTHROPIC_API_KEY": "local-dispatcher"
  }
}
```

Dispatcher 已支持 Anthropic Messages 请求、流式响应、工具调用、智能路由和供应商
回退。Alpha 阶段仍在继续扩大真实账户兼容性测试范围。

上游配置机制可参考 Anthropic 官方的
[LLM 网关文档](https://docs.anthropic.com/en/docs/claude-code/llm-gateway)。

## 支持的模型供应商

| 供应商      | 环境变量                                         |
| ----------- | ------------------------------------------------ |
| Anthropic   | `ANTHROPIC_API_KEY`                              |
| OpenAI      | `OPENAI_API_KEY`                                 |
| Gemini      | `GEMINI_API_KEY`                                 |
| OpenRouter  | `OPENROUTER_API_KEY`                             |
| SiliconFlow | `SILICONFLOW_API_KEY`                            |
| DeepSeek    | `DEEPSEEK_API_KEY`                               |
| Xiaomi MiMo | `MIMO_API_KEY` 或 `XIAOMIMIMO_API_KEY`           |
| Ollama      | `OLLAMA_BASE_URL`，默认 `http://localhost:11434` |

完整环境变量列表见 [`.env.example`](.env.example)。

## 工作原理

```text
客户端请求
    |
    v
协议兼容层
    |
    v
任务分析与能力筛选
    |
    v
质量 / 成本 / 延迟 / 健康度评分
    |
    v
供应商执行、超时与回退
    |
    v
兼容响应 + 本地遥测
```

Dispatcher 会先判断任务类型，排除不满足能力要求的模型，再对候选模型评分，
同时结合健康状态和熔断器做最终选择。路由结果可以通过控制台和遥测 API 查看。

## API

| 接口                        | 用途                       |
| --------------------------- | -------------------------- |
| `GET /v1/health`            | 服务健康状态               |
| `GET /v1/models`            | OpenAI 兼容的模型发现      |
| `GET /v1/providers`         | 供应商能力与健康状态       |
| `GET /v1/telemetry`         | 用量与成本汇总             |
| `POST /v1/chat/completions` | OpenAI 兼容聊天接口        |
| `POST /v1/messages`         | Anthropic 兼容消息接口     |
| `POST /v1/responses`        | Codex/OpenAI Responses API |

示例：

```bash
curl http://localhost:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "介绍一下这个项目"}]
  }'
```

## Codex 路由模式

### Codex 原生路由

使用 `X-Dispatcher-Mode = "auto"` 保留 Codex 原生链路。Dispatcher 会选择模型、
推理强度和速度，不会把请求转换成第三方供应商协议。

### Codex 交接模式实验

Dispatcher 2.0 会在 Codex 原生路由看到可靠限流 header 时记录 quota telemetry；
遇到应急 429 或 `retry-after` 时，才会额外记录 `dispatcher_handoff.v1` 交接包。
该交接包会出现在控制台 telemetry 中，用户可以复制续接提示词，也可以明确点击后通过
`provider-auto` 让备用模型以降级执行模式继续。这个流程不自动承诺 10% 精确余额、
不模拟托管工具，也不会自动切换到第三方模型。

### 跨供应商路由

使用 `provider-auto`，通过 Dispatcher 进程中配置的供应商凭据处理 Responses 请求：

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

该模式不会模拟 OpenAI 托管工具。Codex 客户端的 Bearer Token 和
`ChatGPT-Account-Id` 不会被转发给第三方供应商。

## 路由配置

可以直接使用压缩包中的示例配置：

```bash
./dispatch serve \
  --web-dir ./web/dist \
  --config ./dispatcher.example.toml
```

该文件用于配置路由策略和各任务等级的覆盖规则。也可以替换供应商模型元数据：

```bash
export DISPATCHER_PROVIDER_METADATA=/path/to/provider-models.toml
```

服务默认只监听 `127.0.0.1`。只有在配置了可靠的身份认证和网络访问控制后，
才应通过 `DISPATCHER_BIND_ADDR` 监听其他网卡。

## 从源码构建

环境要求：

- Rust 1.95 或更高版本
- Node.js 22
- pnpm 10

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run --release -- serve --web-dir ./web/dist
```

## 开发与验证

以下命令与 CI 使用的检查一致：

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

欢迎参与贡献。提交 Pull Request 前请阅读
[CONTRIBUTING.md](CONTRIBUTING.md)。安全漏洞请按照
[SECURITY.md](SECURITY.md) 私下报告。

## Alpha 版本限制

- 发布的二进制文件尚未签名。
- 供应商价格和模型能力变化频繁，内置元数据不能作为账单保证。
- Anthropic 原生工具转换仍需更多真实账户测试。
- `provider-auto` 暂不支持网页搜索、图片生成等托管 Responses 工具。
- 当前版本是 CLI 与静态控制台，不是已签名的桌面应用。
- 尚未提供多用户认证和租户隔离。

除非已经部署可靠的网络安全措施，否则请保持默认的本机回环地址。

## 更多文档

- [MVP 使用手册](docs/mvp-user-manual-zh.md)
- [路由研究与产品说明](docs/routing-research-and-product-answer-zh.md)
- [更新记录](CHANGELOG.md)
- [支持说明](SUPPORT.md)

## 开源协议

本项目采用 MIT 协议。版权与来源说明见 [LICENSE](LICENSE) 和
[NOTICE](NOTICE)。
