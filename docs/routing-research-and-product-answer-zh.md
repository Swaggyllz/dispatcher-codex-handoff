# Dispatcher 路由策略调研与产品答案

生成时间：2026-06-03

## 1. 问题

我们刚才发现一个严重问题：如果路由器因为模型 ID 里包含 `flash`、`mini`、`pro` 这类词就加分或降分，这不叫智能路由，只是字符串规则。

这个问题会带来三类风险：

- 模型名称是供应商营销命名，不是稳定能力标签。
- 不同供应商的同一个词含义不同，例如 `pro` 可能是旗舰，也可能只是套餐名。
- 用户会误以为系统在理解任务，实际只是在匹配名字。

所以我们需要调研竞品到底怎么做，并形成 Dispatcher 自己的产品答案。

## 2. 竞品方法概览

### OpenRouter：价格、吞吐、延迟、可用性、fallback

OpenRouter 的 provider routing 文档明确把路由维度放在 provider 级别的运行指标上：

- 默认按价格和可用性做 provider 选择。
- 支持显式按 `price`、`throughput`、`latency` 排序。
- 支持 `preferred_max_latency`、`preferred_min_throughput` 这类性能阈值。
- 支持 provider fallback。
- 对 tools、max tokens 等请求参数做能力过滤。

结论：OpenRouter 的智能重点是 provider 运行质量、价格、性能、能力匹配，不是模型名关键词。

Source: https://openrouter.ai/docs/guides/routing/provider-selection

### LiteLLM：多部署路由、冷却、fallback、tag、pre-call check

LiteLLM 的 Router / Proxy 重点是生产可靠性：

- 路由策略包括 `simple-shuffle`、`least-busy`、`usage-based-routing`、`latency-based-routing`。
- 支持 `enable_pre_call_checks`，调用前检查上下文窗口。
- 支持失败计数、cooldown、retry policy。
- 支持 fallback。
- 支持 tag-based routing。
- 支持成本追踪、预算和多租户。

结论：LiteLLM 更像生产 LLM 网关，路由依据是负载、使用量、延迟、失败状态、上下文窗口和标签，而不是模型名称。

Sources:

- https://docs.litellm.ai/
- https://docs.litellm.com.cn/docs/proxy/config_settings

### Portkey：策略组合，fallback/load-balance/conditional routing/circuit breaker

Portkey 把 AI Gateway 做成可组合策略：

- fallback：按优先级尝试 provider/model，失败后进入下一个。
- load balancing：按权重分流。
- conditional routing：按请求上下文条件分流。
- circuit breaker：对故障做保护。
- fallback target 可以嵌套 load balancer 或 conditional router。
- 日志可以追踪 fallback chain。

结论：Portkey 的强项是策略编排和可观测性。它可以做规则，但规则是显式配置，不伪装成“智能模型名判断”。

Sources:

- https://portkey.ai/docs/product/ai-gateway
- https://portkey.ai/docs/product/ai-gateway/fallbacks
- https://docs1.portkey.ai/docs/product/ai-gateway/load-balancing

### NadirClaw：句向量二分类 + routing modifiers

NadirClaw 是跟 Dispatcher 更近的 agent router。它的 README 里讲得很具体：

- 对 prompt 做 simple/complex 二分类。
- 分类器基于 sentence embeddings。
- 使用预计算 centroid，比较 cosine similarity。
- 低置信度时默认走 complex，宁可过度服务，不要低配复杂任务。
- 有 routing modifiers：
  - 检测 agentic task，强制复杂模型。
  - 检测 reasoning markers。
  - 检测 vision，必要时换成 vision-capable 模型。
  - session persistence，同会话复用模型。
  - context window filtering，超过上下文窗口时换长上下文模型。

结论：NadirClaw 的智能核心是“请求分类 + 安全修正 + 可行性过滤”，不是模型名关键词。

Source: https://github.com/NadirRouter/NadirClaw

### claude-code-router：场景槽位 + 用户自定义 router

claude-code-router 的默认路由更像配置型：

- Router 里有 `default`、`background`、`think`、`longContext`、`webSearch`、`image` 等场景槽位。
- 支持 longContextThreshold。
- 支持 custom router script，用户可以用 JS 自定义复杂规则。
- 支持 transformer，对 provider 协议、tool、reasoning、sampling 等做转换。

结论：CCR 的方式偏“用户声明场景 -> 对应模型”，并把复杂逻辑交给 custom router。它不是自动学习型智能，但也不是默认按模型名营销词路由。

Source: https://github.com/musistudio/claude-code-router

### Kong AI Gateway / TensorZero：语义路由、实验、观测、优化

Kong AI Gateway 文档强调 semantic routing、secure、observe、accelerate、govern。

TensorZero 则是 LLM gateway + observability + optimization + evaluation + experimentation，支持 A/B testing、routing、fallbacks、retries。

结论：这类产品的方向是把路由纳入观测、评估、实验和治理，而不是静态字符串规则。

Sources:

- https://developer.konghq.com/ai-gateway/get-started/
- https://www.tensorzero.com/docs

## 3. 竞品方法分类

竞品方法可以分成五类。

### A. 运维指标路由

典型：OpenRouter、LiteLLM、Portkey。

维度：

- price
- latency
- throughput
- load / least busy
- usage
- failures
- cooldown
- retry
- fallback
- rate limit

优点：可靠、可解释、适合生产。

缺点：不知道任务难度，只知道哪个 provider 当前更划算/更健康。

### B. 请求复杂度分类

典型：NadirClaw、Morph Router、FreeRouter 这类 cost optimizer。

维度：

- prompt 长度
- 对话深度
- 是否代码任务
- 是否多文件/多步骤
- 是否 reasoning
- 是否 agentic task
- embedding classifier
- confidence threshold

优点：真的能省钱，因为简单任务可以走便宜模型。

缺点：分类错了会影响质量，所以需要保守策略。

### C. 能力过滤

典型：OpenRouter、NadirClaw、LiteLLM。

维度：

- tools
- vision
- streaming
- max tokens / context window
- modality
- provider 参数支持

优点：避免把请求发给根本处理不了的模型。

缺点：只能过滤，不负责排序。

### D. 用户显式偏好

典型：claude-code-router、Portkey conditional routing。

维度：

- default/background/think/longContext/image
- custom router script
- allow/deny provider
- explicit fallback chain
- routing profile

优点：用户可控。

缺点：不是自动智能，配置维护成本高。

### E. 评估/反馈驱动

典型：TensorZero，一部分 OpenRouter 高级 router。

维度：

- A/B testing
- eval score
- human feedback
- online telemetry
- model/provider 成功率
- 任务类型下的历史质量

优点：长期最像“智能”。

缺点：实现成本高，需要数据闭环。

## 4. Dispatcher 应该怎么做

Dispatcher 的定位不是通用 LLM Gateway，而是本地 AI coding agent router。

所以我们应该把路线定成：

```text
任务分类 + 能力过滤 + 结构化评分 + session 稳定 + fallback + telemetry 学习
```

不要走：

```text
模型名关键词路由
```

## 5. Dispatcher 路由决策流程建议

### Step 1: Request Analyzer

分析请求本身，而不是模型名：

- token estimate
- message count
- user text length
- system prompt length
- tools present
- image present
- streaming requested
- max_tokens
- task type: chat/code/analysis/translation/summarization
- agent tier: simple/medium/reasoning/complex
- continuation detection

输出：

```text
RequestFeatures
```

### Step 2: Capability Filter

硬过滤不可行模型：

- tools 请求必须支持 tools
- image 请求必须支持 vision
- streaming 请求必须支持 streaming
- token 超上下文窗口直接排除
- max_tokens 超模型输出限制直接排除

输出：

```text
ViableCandidates
```

### Step 3: Objective Scoring

只用结构化指标评分：

- quality_score
- cost_score
- latency_score
- availability_score
- context_fit_score
- reliability_score
- provider_health_score

不要用：

- model_id contains "flash"
- model_id contains "pro"
- model_id contains "opus"
- model_id contains "mini"

### Step 4: Tier Policy Weights

policy 只调权重，不指定模型名：

```toml
[tier_policies.simple]
quality_weight = 0.10
cost_weight = 0.60
latency_weight = 0.15

[tier_policies.reasoning]
quality_weight = 0.65
cost_weight = 0.10
latency_weight = 0.10
```

### Step 5: Safety Modifiers

这部分不是模型名，而是请求风险：

- 有 tools：提高 capability 和 reliability 权重
- 长上下文：提高 context_fit 权重
- 多轮 agent loop：提高 sticky/session 权重
- 低分类置信度：上调 tier，而不是贪便宜
- provider 最近失败：降权或 cooldown

### Step 6: Sticky Session

同一 session 下短确认消息，例如：

- 继续
- ok
- do it
- run

应该复用上一次路线，避免 coding agent 在连续工作流中突然换模型。

### Step 7: Fallback

fallback 不能只选最便宜。

应该按兼容性排序：

1. 同 provider/model family 的健康候选
2. 同 tier 的相近质量候选
3. 能力兼容候选
4. 成本更低候选

### Step 8: Telemetry Learning

MVP 后的真正智能来自数据：

- provider success rate
- provider error rate
- p50/p90 latency
- cost per request
- fallback rate
- task tier 下的失败率
- 用户手动 override 次数

长期可以形成：

```text
effective_quality_score = base_quality + task_success_signal - failure_penalty
```

## 6. Dispatcher 的产品答案

### 一句话

Dispatcher 是一个面向 AI coding agent 的本地智能路由器：它先理解请求，再过滤不可用模型，然后基于质量、成本、延迟、上下文、可靠性和历史表现选路由。

### 对用户怎么讲

不要说：

```text
我们根据模型名判断哪个模型便宜/强。
```

要说：

```text
我们根据请求复杂度、模型能力、成本、延迟、上下文窗口、provider 健康度和历史表现做路由。
```

### 对工程怎么做

MVP：

- 保留 AgentTier
- 保留 Capability Filtering
- 保留 Tier 权重
- 删除模型名关键词评分
- policy 不再预设模型关键词
- routing metadata 显示：
  - agent_tier
  - decision_reason
  - capability_filter_reason
  - top_candidate_scores

下一版：

- provider health check 进入评分
- telemetry 进入评分
- p50/p90 latency 进入评分
- fallback chain 可解释
- route simulation 页面

再下一版：

- embedding classifier 或小模型分类器
- confidence threshold
- 用户反馈修正
- per-task eval 数据

## 7. 我们现在已经修正的点

已经做掉：

- scorer 不再按模型名里的 `flash`、`mini`、`pro`、`opus` 加减分。
- `dispatcher.example.toml` 不再预设模型关键词。
- 新增测试保证同等结构化指标的模型不会因名称不同而得分不同。

还应该继续做：

- 把 `preferred_model_keywords` / `avoided_model_keywords` 从 public docs 中弱化或移除。
- 给每个 routing decision 输出更详细的 score breakdown。
- 加一个 Dashboard “为什么选它” 面板。

## 8. 已开始落地：Explainable Routing Decision

本轮已经完成第一版：

- `ProviderScore` 增加 `score_breakdown`，展示质量、成本、延迟、可用性的加权贡献。
- `RoutingDecision` 增加 `decision_reason`。
- `/v1/chat/completions` 的 `routing` metadata 增加：
  - `decision_reason`
  - `top_candidates`
  - `top_candidates[].score_breakdown`
- Dashboard Quick Test 增加：
  - 决策原因
  - Top candidates 候选排序表
  - 每个候选的总分、质量贡献、成本贡献、延迟贡献

下一步继续增强：

- 展示被排除候选及原因，例如 tools unsupported、context too short、streaming unsupported。
- provider health check 进入评分。
- telemetry 里的真实延迟、成功率、fallback 率进入评分。
- 做 Route Simulation 页面，在不发真实请求的情况下模拟一次路由。
