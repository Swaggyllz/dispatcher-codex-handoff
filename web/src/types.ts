// ============================================================
// Dispatcher Dashboard — API 响应类型
// ============================================================

// GET /v1/health
export interface HealthStatus {
  status: "ok" | "degraded" | "error";
  version: string;
}

// GET /v1/providers
export interface ProvidersResponse {
  providers: ProviderInfo[];
}

export interface ProviderInfo {
  id: string;
  name: string;
  models: ModelInfo[];
  supports_streaming: boolean;
  supports_tools: boolean;
  supports_vision: boolean;
  health: ProviderHealth;
}

export interface ProviderHealth {
  status: "healthy" | "degraded" | "down" | "unknown";
  sample_count: number;
  success_rate: number | null;
  avg_latency_ms: number | null;
  circuit_state: "closed" | "open" | "half_open";
  failure_count: number;
  cooldown_remaining_secs: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  cost: {
    input_per_1k: number;
    output_per_1k: number;
  };
  pricing_source: string | null;
  pricing_updated_at: string | null;
  supports_streaming: boolean;
  supports_tools: boolean;
  supports_vision: boolean;
  max_tokens: number;
  quality_score: number;
  avg_latency_ms: number;
}

// GET /v1/telemetry
export interface TelemetryStats {
  total_requests: number;
  total_success: number;
  total_tokens: number;
  total_cost_usd: number;
  avg_latency_ms: number;
  success_rate: number;
  cost_summary: CostSummary;
  cost_by_tier: TierCostBreakdown[];
  cost_by_strategy: StrategyCostBreakdown[];
  provider_stats: ProviderStat[];
  latest_codex_route: CodexRouteTelemetry | null;
  latest_quota_event: QuotaEventTelemetry | null;
  latest_handoff: HandoffPackageTelemetry | null;
  latest_handoff_continuation: HandoffContinuationTelemetry | null;
}

export interface CostSummary {
  today_usd: number;
  month_usd: number;
  total_usd: number;
}

export interface CostBreakdown {
  total_requests: number;
  total_tokens: number;
  total_cost_usd: number;
}

export interface TierCostBreakdown extends CostBreakdown {
  agent_tier: AgentTier;
}

export interface StrategyCostBreakdown extends CostBreakdown {
  routing_strategy: string;
}

export interface CodexRouteTelemetry {
  timestamp: string;
  requested_model: string;
  model: string;
  reasoning_effort: "low" | "medium" | "high" | "xhigh";
  speed: "standard" | "priority";
  agent_tier: AgentTier;
  reason: string;
  success: boolean;
  status_code: number | null;
  latency_ms: number;
  error_message: string | null;
}

export interface QuotaEventTelemetry {
  timestamp: string;
  provider_id: string;
  model_id: string;
  status_code: number | null;
  retry_after_secs: number | null;
  normalized_headroom: number | null;
  source: string;
}

export interface HandoffPackageTelemetry {
  schema_version: "dispatcher_handoff.v1";
  package_id: string;
  created_at: string;
  trigger: "planned" | "quota_warning" | "rate_limit_429" | "manual";
  confidence: "strong_summary" | "emergency_reconstruction";
  objective: string;
  latest_user_request: string;
  current_status: string;
  completion_criteria: string[];
  workspace: HandoffWorkspaceTelemetry;
  execution_state: HandoffExecutionStateTelemetry;
  technical_context: HandoffTechnicalContextTelemetry;
  routing_context: HandoffRoutingContextTelemetry;
  continuation_prompt: string;
  hazards: string[];
  open_questions: string[];
}

export interface HandoffWorkspaceTelemetry {
  cwd: string;
  repo_name: string | null;
  branch: string | null;
  dirty_state: string;
  touched_files: string[];
  relevant_files: string[];
}

export interface HandoffExecutionStateTelemetry {
  mode: "plan_only" | "research_only" | "edit_allowed" | "verify_only";
  last_successful_step: string | null;
  next_recommended_step: string;
  blocked_on: string | null;
  commands_run: string[];
  verification_run: string[];
}

export interface HandoffTechnicalContextTelemetry {
  key_findings: string[];
  decisions_made: string[];
  assumptions: string[];
  constraints: string[];
}

export interface HandoffRoutingContextTelemetry {
  agent_tier: AgentTier;
  requested_model: string;
  selected_model: string;
  reasoning_effort: "low" | "medium" | "high" | "xhigh";
  speed: "standard" | "priority";
  dispatcher_mode: string;
}

export interface HandoffContinuationTelemetry {
  timestamp: string;
  package_id: string;
  provider_id: string;
  model_id: string;
  success: boolean;
  status_code: number | null;
  latency_ms: number;
  response_text: string | null;
  error_message: string | null;
  review_prompt: string;
}

export interface ProviderStat {
  provider_id: string;
  total_requests: number;
  avg_latency_ms: number;
  success_count: number;
  request_tokens: number;
  response_tokens: number;
  total_tokens: number;
  total_cost_usd: number;
  model_stats?: ModelStat[];
}

export interface ModelStat {
  model_id: string;
  total_requests: number;
  avg_latency_ms: number;
  success_count: number;
  request_tokens: number;
  response_tokens: number;
  total_tokens: number;
  total_cost_usd: number;
}

// GET /v1/policy
export interface RoutingPolicy {
  default_strategy: "auto" | "save" | "fast" | "manual" | "random";
  fallback_enabled: boolean;
  circuit_breaker_threshold: number;
  circuit_breaker_timeout_secs: number;
  strategies: StrategyPolicy[];
  tiers: TierPolicy[];
  editable: boolean;
  config_path: string | null;
  editable_policy: PolicyUpdate | null;
}

export interface StrategyPolicy {
  strategy: "auto" | "save" | "fast";
  weights: EffectivePolicyWeights;
  overridden: boolean;
}

export interface TierPolicy {
  tier: AgentTier;
  weights: EffectivePolicyWeights;
  preferred_model_keywords: string[];
  avoided_model_keywords: string[];
  overridden: boolean;
}

export interface EffectivePolicyWeights {
  quality: EffectivePolicyWeight;
  cost: EffectivePolicyWeight;
  latency: EffectivePolicyWeight;
  availability: EffectivePolicyWeight;
}

export interface EffectivePolicyWeight {
  value: number;
  overridden: boolean;
}

export interface PolicyUpdate {
  default_strategy: "auto" | "save" | "fast";
  fallback_enabled: boolean;
  circuit_breaker_threshold: number;
  circuit_breaker_timeout_secs: number;
  strategy_weights: Record<RoutingStrategy, PolicyWeights>;
  tier_policies: Partial<Record<AgentTier, EditableTierPolicy>>;
}

export interface PolicyWeights {
  quality: number;
  cost: number;
  latency: number;
  availability: number;
}

export interface EditableTierPolicy {
  quality_weight: number | null;
  cost_weight: number | null;
  latency_weight: number | null;
  preferred_model_keywords: string[];
  avoided_model_keywords: string[];
}

export interface PolicyValidationField {
  field: string;
  message: string;
}

export interface PolicySaveResponse {
  saved: boolean;
  restart_required: boolean;
  config_path: string;
  saved_policy: RoutingPolicy;
}

// POST /v1/chat/completions
export interface ChatCompletionResponse {
  id?: string;
  provider?: string;
  model?: string;
  choices?: Choice[];
  usage?: Usage;
  routing?: RoutingMetadata;
  error?: { message: string; type?: string };
}

export interface Choice {
  index?: number;
  message?: { role: string; content: string };
  finish_reason?: string;
}

export interface Usage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// POST /v1/responses with X-Dispatcher-Mode: provider-auto
export interface ProviderContinuationResponse {
  id?: string;
  object?: string;
  status?: string;
  model?: string;
  dispatcher_provider?: string | null;
  dispatcher_model?: string | null;
  output?: ResponsesOutputItem[];
  error?: { message: string; type?: string } | null;
  usage?: {
    input_tokens?: number;
    output_tokens?: number;
    total_tokens?: number;
  };
}

export interface ResponsesOutputItem {
  type: string;
  role?: string;
  status?: string;
  content?: ResponsesContentPart[];
  name?: string;
  arguments?: string;
}

export interface ResponsesContentPart {
  type: string;
  text?: string;
}

export interface RoutingMetadata {
  provider: string;
  strategy: string;
  agent_tier: AgentTier;
  is_fallback: boolean;
  fallback_reason?: string | null;
  fallback_chain?: RouteAttempt[];
  policy_reason?: string | null;
  decision_reason?: string;
  top_candidates?: RoutingCandidate[];
  excluded_candidates?: ExcludedCandidate[];
  decision_time_ms?: number;
}

export interface RoutingCandidate {
  provider: string;
  model: string;
  total_score: number;
  quality_score: number;
  cost_score: number;
  latency_score: number;
  availability_score: number;
  estimated_cost_per_1k: number;
  avg_latency_ms: number;
  availability?: "available" | "degraded" | "unavailable";
  policy_reason?: string | null;
  score_breakdown: ScoreBreakdown;
  input_cost_per_1k: number;
  output_cost_per_1k: number;
}

export interface RouteAttempt {
  provider_id: string;
  model_id: string;
  status: "success" | "failed";
  error?: string | null;
}

export interface ScoreBreakdown {
  weighted_quality: number;
  weighted_cost: number;
  weighted_latency: number;
  weighted_availability: number;
  weighted_policy: number;
}

export interface ExcludedCandidate {
  provider_id: string;
  model_id?: string | null;
  reason: string;
}

// Routing strategy
export type RoutingStrategy = "auto" | "save" | "fast";
export type AgentTier = "simple" | "medium" | "reasoning" | "complex";
