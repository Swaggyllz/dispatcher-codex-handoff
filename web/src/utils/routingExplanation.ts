import type { ExcludedCandidate, RoutingMetadata } from "@/types";

type Language = "zh" | "en";

const normalizeLanguage = (language?: string): Language =>
  language?.toLowerCase().startsWith("zh") ? "zh" : "en";

const tierLabel = (tier: string, language: Language) => {
  if (language === "en") return tier;
  const labels: Record<string, string> = {
    simple: "简单",
    medium: "中等",
    reasoning: "推理",
    complex: "复杂",
  };
  return labels[tier] ?? tier;
};

export function formatDecisionReason(
  routing: RoutingMetadata,
  selectedModel: string | undefined,
  language?: string,
) {
  const lang = normalizeLanguage(language);
  if (lang === "en") {
    return routing.decision_reason ?? "";
  }

  const selected = `${routing.provider}/${selectedModel ?? "unknown"}`;
  if (routing.fallback_reason === "sticky_session_continuation") {
    return `这是一个短确认或继续请求，Dispatcher 复用了上一轮路由 ${selected}，避免连续任务中频繁切换模型。`;
  }

  if (routing.is_fallback) {
    return `主路线不可用，Dispatcher 触发故障转移，改用 ${selected}。`;
  }

  const top = routing.top_candidates?.[0];
  if (!top) {
    return `Dispatcher 将请求判定为「${tierLabel(routing.agent_tier, lang)}」层级，并选择 ${selected}。`;
  }

  const isSaveStrategy =
    routing.strategy.toLowerCase() === "save" ||
    routing.decision_reason?.includes("lowest estimated cost");
  const isFastStrategy =
    routing.strategy.toLowerCase() === "fast" ||
    routing.decision_reason?.includes("lowest latency");
  const usesNearEqualTieBreak =
    routing.decision_reason?.includes("near-equal costs");
  const usesNearEqualLatencyTieBreak =
    routing.decision_reason?.includes("near-equal latency");
  const basis = isSaveStrategy
    ? usesNearEqualTieBreak
      ? "按任务难度质量达标、且价格接近时综合表现更好的"
      : "按任务难度质量达标后预估成本最低的"
    : isFastStrategy
      ? usesNearEqualLatencyTieBreak
        ? "按任务难度质量达标、且延迟接近时综合表现更好的"
        : "按任务难度质量达标后预估延迟最低的"
      : "综合得分最高的";

  return `Dispatcher 将请求判定为「${tierLabel(routing.agent_tier, lang)}」层级，并选择${basis} ${selected}。本次评分比较质量、成本、延迟和可用性：总分 ${top.total_score.toFixed(3)}，质量贡献 ${top.score_breakdown.weighted_quality.toFixed(3)}，成本贡献 ${top.score_breakdown.weighted_cost.toFixed(3)}，延迟贡献 ${top.score_breakdown.weighted_latency.toFixed(3)}。`;
}

export function formatSelectionBasis(
  routing: RoutingMetadata,
  language?: string,
) {
  const lang = normalizeLanguage(language);
  const strategy = routing.strategy.toLowerCase();
  const isSaveStrategy =
    strategy === "save" ||
    routing.decision_reason?.includes("lowest estimated cost");
  const isFastStrategy =
    strategy === "fast" || routing.decision_reason?.includes("lowest latency");
  const usesNearEqualTieBreak =
    routing.decision_reason?.includes("near-equal costs");
  const usesNearEqualLatencyTieBreak =
    routing.decision_reason?.includes("near-equal latency");

  if (lang === "en") {
    if (isFastStrategy) {
      return usesNearEqualLatencyTieBreak
        ? "Tier-aware quality gate + near-equal latency tie-break"
        : "Tier-aware quality gate + lowest estimated latency";
    }
    if (!isSaveStrategy) return "Best overall fit";
    return usesNearEqualTieBreak
      ? "Tier-aware quality gate + near-equal cost tie-break"
      : "Tier-aware quality gate + lowest estimated cost";
  }

  if (isFastStrategy) {
    return usesNearEqualLatencyTieBreak
      ? "按任务难度质量达标 + 近似同延迟选综合更好"
      : "按任务难度质量达标 + 延迟优先";
  }
  if (!isSaveStrategy) return "综合表现最高";
  return usesNearEqualTieBreak
    ? "按任务难度质量达标 + 近似同价选综合更好"
    : "按任务难度质量达标 + 成本优先";
}

export function formatSelectedRoute(
  routing: RoutingMetadata,
  selectedModel?: string,
) {
  return `${routing.provider}/${selectedModel ?? "unknown"}`;
}

export function formatExclusionReason(
  candidate: ExcludedCandidate,
  language?: string,
) {
  const lang = normalizeLanguage(language);
  if (lang === "en") return candidate.reason;

  if (candidate.reason === "tools unsupported") return "不支持工具调用";
  if (candidate.reason === "vision unsupported") return "不支持图片/视觉输入";
  if (candidate.reason === "streaming unsupported") return "不支持流式响应";
  if (candidate.reason === "provider circuit breaker open")
    return "供应商熔断中，暂时跳过";
  if (candidate.reason.startsWith("context too short")) return "上下文窗口不足";

  return candidate.reason;
}
