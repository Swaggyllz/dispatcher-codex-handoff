# Dispatcher MVP 使用说明书

## 1. 它是干嘛的

Dispatcher 是一个跑在你电脑本地的 AI 模型路由器。

核心用法是把 Codex 桌面端的 Local / Worktree 请求接到 Dispatcher，由
Dispatcher 自动选择原生 Codex 模型、推理力度和速度。Claude Code、Cursor 和
其他兼容 OpenAI/Anthropic API 的工具也可以使用相应的次级路由通道。

一句话：

```text
Codex Desktop Local/Worktree -> Dispatcher -> gpt-5.5 / gpt-5.4 / gpt-5.4-mini
其他 AI 编程工具          -> Dispatcher -> 对应原生或多 provider 路由池
```

它现在的重点不是“聊天机器人”，而是“给 AI 编程代理省钱、提稳定性、选对模型”。

## 2. 它解决什么问题

AI 编程代理的请求差异很大：

- “继续”“ok”“好的”这种请求，不需要旗舰模型
- 简单解释、短摘要，可以用便宜模型
- 写代码、debug、架构分析，需要强模型
- 带 tools、vision、streaming、长上下文的请求，必须过滤掉不支持能力的 provider
- 主 provider 失败时，最好能 fallback 到其他 provider
- 同一段 agent 工作流里，短确认应该保持上一轮模型，避免上下文和能力突然切换

Dispatcher 做的就是这些判断。

## 3. 当前 MVP 能做什么

当前 MVP 已经支持：

- Dashboard 页面
- `/v1/chat/completions` OpenAI 兼容接口
- `/v1/messages` Anthropic 兼容入口
- `/v1/responses` Codex 桌面端 / CLI / IDE 共用的 Responses API 入口
- provider 能力展示
- Quick Test 快速测试
- 请求分层：`simple` / `medium` / `reasoning` / `complex`
- tier-aware 路由评分
- 可配置 tier policy
- Dashboard 双语策略编辑、服务端校验和原子持久化
- provider 能力过滤：tools、vision、streaming、context length
- fallback
- sticky session continuation
- telemetry 记录
- 本地 Demo Provider，不需要任何 API Key 就能测试完整链路

## 4. 现在怎么测试

你当前这个 MVP 服务已经跑在：

```text
http://localhost:8788
```

打开 Dashboard：

```text
http://localhost:8788
```

在 Quick Test 输入：

```text
hello demo
```

预期会返回类似：

```text
[demo] Dispatcher received your prompt and routed it locally.

Prompt: hello demo
```

你应该能看到 routing 信息：

```text
provider: demo
model: demo-echo
agent_tier: simple
policy_reason: simple policy: tier weights override
```

这说明完整链路已经通了：

```text
Dashboard -> Dispatcher API -> Analyzer -> Scorer -> Selector -> Demo Provider -> Response
```

## 5. 没有真实模型也能测试吗

可以。

MVP 里有一个本地 Demo Provider。它不访问外网，不需要 Anthropic、OpenAI、Ollama，也不需要任何 API Key。

启动方式：

```bash
cd /path/to/dispatcher

cargo run -- serve \
  --port 8788 \
  --web-dir ./web/dist \
  --config ./dispatcher.example.toml
```

Demo Provider 默认启用。它的作用是验证产品链路，不是替代真实大模型。它会 echo 你的 prompt，并返回 routing metadata。

如需关闭：

```bash
DISPATCHER_DEMO_PROVIDER=0 cargo run -- serve --port 8788 --web-dir ./web/dist
```

## 6. 怎么接真实模型

设置你有的 API Key，然后启动服务。

例如 OpenAI：

```bash
export OPENAI_API_KEY="sk-xxx"
```

例如 Gemini：

```bash
export GEMINI_API_KEY="xxx"
```

例如 OpenRouter：

```bash
export OPENROUTER_API_KEY="sk-xxx"
```

例如 DeepSeek：

```bash
export DEEPSEEK_API_KEY="xxx"
```

例如 SiliconFlow：

```bash
export SILICONFLOW_API_KEY="xxx"
```

例如小米 MiMo：

```bash
export XIAOMIMIMO_API_KEY="xxx"
```

然后启动：

```bash
cargo run -- serve \
  --port 8788 \
  --web-dir ./web/dist \
  --config ./dispatcher.example.toml
```

Demo Provider 默认会和真实 Provider 一起参与兼容路由，并在外部 Provider 不可用时提供本地兜底。

## 7. 怎么接 Claude Code

把 Claude Code 的 Anthropic base URL 指向 Dispatcher：

```bash
ANTHROPIC_BASE_URL=http://localhost:8788 \
ANTHROPIC_API_KEY=local-dispatcher \
claude
```

如果写到 `~/.claude/settings.json`：

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://localhost:8788",
    "ANTHROPIC_API_KEY": "local-dispatcher"
  }
}
```

之后 Claude Code 发出的请求会先进入 Dispatcher，再由 Dispatcher 路由到合适 provider/model。

注意：Claude Code 的 `ANTHROPIC_BASE_URL` 使用服务根地址
`http://localhost:8788`，不要加 `/v1`。Claude Code 会自行拼接
`/v1/messages`。

## 8. 怎么接 Codex 桌面端 / Cursor / OpenAI 兼容工具

任何支持 OpenAI-compatible API 的工具，都可以把 base URL 指到：

```text
http://localhost:8788/v1
```

普通 OpenAI Chat Completions 兼容工具常见环境变量：

```bash
export OPENAI_BASE_URL=http://localhost:8788/v1
```

有些工具叫：

```bash
export OPENAI_API_BASE=http://localhost:8788/v1
```

具体变量名要看对应工具文档。

Codex 桌面端、CLI 和 IDE 使用同一套 agent 配置。Dispatcher 的目标入口是
Codex 桌面端 Local / Worktree 线程；CLI 主要用于自动化协议验收。

桌面端有两种登录方式：

1. ChatGPT Plus / Pro / Business 等订阅登录：Dispatcher 的主路径
2. OpenAI API Key 登录：兼容和备用路径

用户级 `~/.codex/config.toml` 使用：

```toml
model = "gpt-5.5"
model_provider = "dispatcher"

[model_providers.dispatcher]
name = "Dispatcher"
base_url = "http://localhost:8788/v1"
wire_api = "responses"
requires_openai_auth = true
http_headers = { "X-Dispatcher-Mode" = "auto" }
```

`requires_openai_auth = true` 会复用 Codex 桌面端现有的 ChatGPT 登录态或 API
Key 登录态，不需要本地占位 Key。订阅请求会同时携带 bearer token 和
`ChatGPT-Account-Id`；Dispatcher 只做内存转发，不保存或展示这些值。

鉴权优先级：

1. 桌面端 ChatGPT 订阅登录态
2. Dispatcher 服务进程中的专属 API Key
3. Dispatcher 服务进程中的通用 OpenAI API Key
4. 桌面端转发的 API Key 登录态

如需使用服务端 API Key 备用路径：

```bash
export DISPATCHER_CODEX_API_KEY="sk-xxx"
# 如果没有设置专属 Key，也可以使用：
# export OPENAI_API_KEY="sk-xxx"
```

如需接入兼容 Responses API 的测试或私有端点，可设置
`DISPATCHER_CODEX_BASE_URL`；默认值是 `https://api.openai.com/v1`。

Codex 的最终路由策略不是在所有 provider 之间混选，而是使用专属原生通道：

```text
gpt-5.5 / gpt-5.4 / gpt-5.4-mini
        × 推理力度
        × 速度档位
```

当前路由矩阵：

| 任务层级 | 模型 | 默认推理力度 |
|----------|------|----------------|
| simple | `gpt-5.4-mini` | `low` |
| medium | `gpt-5.4` | `medium` |
| reasoning | `gpt-5.5` | `high` |
| complex | `gpt-5.5` | `xhigh` |

自动模式由 `X-Dispatcher-Mode: auto` 开启。Codex 桌面端仍显示并使用它认识的
`gpt-5.5` 模型元数据，Dispatcher 会忽略请求中的默认模型、默认推理力度和默认
速度，再按最新用户意图同时决定三项。未启用该 Header 时，显式锁定模式继续尊重
用户选择。
`service_tier = "priority"` 对 `gpt-5.5` 和 `gpt-5.4` 生效；
`gpt-5.4-mini` 不支持 priority，因此自动回落到 standard。ChatGPT 订阅账号若
不支持 fast，Dispatcher 会自动重试 standard，不会让整轮请求失败。

`/v1/responses` 已改为 Codex 原生 Responses API 透传，保留未知字段、内置
工具、function tool、reasoning summary 和流式事件。响应头会返回最终的
`X-Dispatcher-Codex-Model`、`X-Dispatcher-Reasoning-Effort` 和
`X-Dispatcher-Speed`，便于调试和后续 Dashboard 展示。

### 8.1 在 Codex 桌面端使用其他模型

要让 Codex 桌面端从 Anthropic、DeepSeek、OpenRouter 等已配置 provider 中
自动选模型，只需把同一份配置中的 Header 改为：

```toml
web_search = "disabled"

[features]
image_generation = false

[model_providers.dispatcher]
name = "Dispatcher Multi-provider"
base_url = "http://localhost:8788/v1"
wire_api = "responses"
requires_openai_auth = true
http_headers = { "X-Dispatcher-Mode" = "provider-auto" }
```

`provider-auto` 会把 Responses API 的文本、流式输出和 function tool 调用转换成
Dispatcher 内部的统一请求，再按任务层级、能力、健康状态和回退策略选择 provider。
它只使用 Dispatcher 服务进程中配置的供应商 Key；Codex 桌面端携带的 ChatGPT
token 和 `ChatGPT-Account-Id` 不会转发给第三方 provider。

第一版边界：

- 支持文本输入、流式文本和 function tool
- Codex 客户端附带的 `custom` 和 `tool_search` 定义会被忽略，标准 function tools
  仍会转发
- 不模拟 OpenAI 托管工具；使用 `provider-auto` 时需要按上方配置关闭
  `web_search` 和 image generation
- 不转换 OpenAI reasoning summary
- 只有声明支持当前请求所需 streaming/tools 能力的 provider 才会进入候选集
- `auto` 和 `provider-auto` 是两条独立通道，可通过 Header 随时切换

## 9. 路由是怎么判断的

Dispatcher 会先分析请求：

- 最新真实用户意图中的代码/分析/翻译等任务类型
- 最新真实用户意图的复杂度和 agent tier
- 完整请求的 token/context 大小
- 是否带工具
- 是否带图片
- 是否流式
- 是否长上下文

AI 编程代理每轮附带的长 system/developer 说明、完整工具目录、合成环境块和工具结果
不会抬高任务档位；但这些内容仍计入上下文长度，tools/vision 仍用于能力过滤。

然后按 tier 评分：

| Tier | 典型请求 | 路由倾向 |
|------|----------|----------|
| simple | 问候、确认、短问答 | 便宜、快速 |
| medium | 单步代码/文本任务 | 均衡 |
| reasoning | debug、架构、复杂代码 | 强推理/强代码 |
| complex | 多 agent、多工作流、并行任务 | 旗舰模型 |

最后选总分最高的候选模型。

## 10. 怎么改路由策略

在 Dashboard 的“路由策略”面板点击编辑按钮，可以修改：

- 默认 Auto / Save / Fast 模式
- fallback
- 熔断阈值和恢复时间
- 三种模式的质量、成本、延迟、可用性权重
- 四个任务层级的可选覆盖和模型关键词

服务端会校验每行权重合计 100%、数值范围、支持的策略、关键词和熔断参数。未知字段
会被拒绝，策略 API 不读取、返回或写入 API Key、ChatGPT token 或账号 id。

保存会原子写入：

```text
/path/to/dispatcher/dispatcher.toml
```

保存成功后 Dashboard 会明确提示需要重启；运行中的引擎不会假装已经热更新。也可以
手工编辑 `dispatcher.example.toml` 的副本，例如：

```toml
[tier_policies.simple]
quality_weight = 0.10
cost_weight = 0.60
latency_weight = 0.15
```

含义：

- `quality_weight`：质量权重
- `cost_weight`：成本权重
- `latency_weight`：延迟权重
- 路由依据是结构化指标：质量分、成本、延迟、上下文长度、能力过滤和可用性
- 默认智能策略不按模型名称关键词加分或降分

改完后重启 Dispatcher 生效。

## 11. 常用命令

启动 demo MVP：

```bash
cargo run -- serve --port 8788 --web-dir ./web/dist --config ./dispatcher.example.toml
```

设置单次 provider 连接/初始响应超时：

```bash
DISPATCHER_PROVIDER_TIMEOUT_SECS=30 cargo run -- serve --port 8788 --web-dir ./web/dist
```

默认值是 30 秒，可配置范围为 1-300 秒。

查看健康状态：

```bash
curl http://localhost:8788/v1/health
```

查看 provider：

```bash
curl http://localhost:8788/v1/providers
```

CLI 查看配置和 provider：

```bash
cargo run -- config --config ./dispatcher.example.toml
```

CLI 路由一次请求：

```bash
cargo run -- route \
  --model auto \
  --prompt "hello demo" \
  --strategy auto \
  --config ./dispatcher.example.toml
```

## 12. 常见问题

### Dashboard 有 provider，但请求失败

可能是 provider 被注册了，但真实模型不可用。例如 Ollama 默认会显示本地模型，但如果你没有安装/拉取对应模型，请求会失败。

解决办法：

- 先确认默认启用的 Demo Provider 能返回结果
- 或者配置真实 API Key
- 或者安装并拉取 Ollama 模型

### 没有任何 API Key 怎么测

用 Demo Provider：

```bash
cargo run -- serve --port 8788 --web-dir ./web/dist --config ./dispatcher.example.toml
```

### 为什么我的 simple 请求路由到某个 provider

默认策略不会因为模型名里有 `flash`、`pro`、`demo` 之类的词就加分。simple 请求主要看低成本、低延迟、足够质量和 provider 可用性。

如果结果不符合预期，应该优先检查模型的成本、延迟、质量分和真实可用性，而不是给模型名写关键词规则。

### 8788 被占用了怎么办

换端口：

```bash
cargo run -- serve --port 8790 --web-dir ./web/dist --config ./dispatcher.example.toml
```

然后访问：

```text
http://localhost:8790
```

## 13. 当前 MVP 的边界

通用功能 MVP、Codex 桌面订阅链路、Dashboard 和策略编辑已完成本地验收。

当前边界：

- Responses API 已使用原生透传覆盖文本流、function tool、内置工具字段、
  reasoning summary 和完整 Responses 对象
- `provider-auto` 已支持将 Codex 的文本流和 function tool 路由到通用
  provider 池；OpenAI 托管工具和 reasoning summary 暂不做跨厂商模拟
- Codex CLI 已完成协议验收，但这只证明 Responses API 和工具调用兼容
- Codex 桌面端 ChatGPT 订阅自动模式、鉴权转发和真实工具调用已经验收
- 单独的 OpenAI API Key 计费路径真实验收仍是可选补充
- policy 配置需要重启后生效
- Dashboard 已可编辑 policy，并明确显示重启要求
- telemetry 还是本地 SQLite 简单记录
- provider 模型列表还是预设为主

下一阶段优先做 provider/model 价格与能力元数据外置，以及成本统计细化。

## 14. MVP 之后的方向：Planner + Router

当前 MVP 是“单次请求路由器”：用户发来一次请求，Dispatcher 判断任务层级，然后选择合适的 provider/model。

MVP 稳定之后，可以升级成“任务拆解器 + 路由器”：

```text
用户目标 -> 拆成步骤 -> 每一步判断难度 -> 分别路由到合适模型 -> 校验结果 -> 继续下一步
```

例如用户说“帮我做一个项目”，Dispatcher 不应该把整个目标一次性丢给一个模型，而是拆成：

- 需求理解
- 项目结构设计
- 信息抓取、文件读取、数据对比
- 核心代码实现
- 测试补充
- 错误修复
- 总结变更

每一步可以走不同模型：

- 抓取信息、读取文件、对比 JSON/CSV、整理表格：中等或快速便宜模型
- 写小函数、适配器、配置、普通测试：中等模型
- 架构设计、核心实现、跨文件重构、疑难 bug、安全/错误处理审查：推理模型
- 多个子任务并行、前后端/安全多工作流协作：复杂编排层，再对子步骤分别路由

边界：

- 这不是当前 MVP 必须完成的功能
- 当前阶段先把单次开发者任务路由做好
- Planner + Router 是下一阶段产品路线，不是普通聊天机器人方向
