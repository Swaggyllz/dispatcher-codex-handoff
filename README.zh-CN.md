# Dispatcher

面向 AI 编程智能体的本地智能模型路由器。

简体中文 | [English](README.md)

> **Alpha 版本：** `v0.1.0-alpha.1` 已可用于本地体验。`v1.0` 之前，
> 配置格式和模型供应商元数据仍可能调整。

Dispatcher 在你的电脑上运行一个兼容 OpenAI API 的服务。它会分析每个请求，
结合质量、成本、延迟、能力和近期健康状态选择供应商与模型，并记录可解释的
路由决策。

## 为什么使用 Dispatcher？

- 为 Codex、OpenAI 兼容客户端和 Anthropic 客户端提供统一的本地地址
- 自动识别 `simple`、`medium`、`reasoning`、`complex` 四类任务
- 提供 `auto`、`save`、`fast` 三种路由策略
- 支持健康评分、熔断、超时保护和自动回退
- 根据工具调用、视觉、流式输出和上下文窗口筛选模型
- 本地控制台展示路由解释、用量、延迟和成本
- 默认内置演示供应商，无需 API Key 也能跑通完整流程

## 5 分钟快速上手

### 1. 下载

从 [v0.1.0-alpha.1](https://github.com/Swaggyllz/dispatcher/releases/tag/v0.1.0-alpha.1)
下载对应系统的压缩包：

| 系统             | 文件                              |
| ---------------- | --------------------------------- |
| macOS Apple 芯片 | `dispatcher-macos-aarch64.tar.gz` |
| Linux x86_64     | `dispatcher-linux-x86_64.tar.gz`  |
| Windows x86_64   | `dispatcher-windows-x86_64.zip`   |

目前发布的二进制文件尚未签名，系统可能要求你确认信任下载的文件。

### 2. 启动 Dispatcher

macOS 或 Linux：

```bash
tar -xzf dispatcher-*.tar.gz
./dispatch serve --web-dir ./web/dist
```

Windows PowerShell：

```powershell
Expand-Archive .\dispatcher-windows-x86_64.zip -DestinationPath .\dispatcher
cd .\dispatcher
.\dispatch.exe serve --web-dir .\web\dist
```

### 3. 打开控制台

访问 [http://localhost:8787](http://localhost:8787)。API 地址为
`http://localhost:8787/v1`。

首次启动不需要配置模型供应商密钥，可以直接在控制台中使用演示供应商测试路由。

### 4. 接入真实模型供应商

启动前设置一个或多个供应商密钥：

```bash
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
./dispatch serve --web-dir ./web/dist
```

不要把真实密钥提交到 Git。Dispatcher 从服务进程的环境变量读取凭据，
不会自动加载 `.env` 文件。

## 接入 Codex

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
