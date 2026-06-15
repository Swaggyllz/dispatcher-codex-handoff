import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Loader2, Route } from "lucide-react";
import { StrategySelector } from "@/components/StrategySelector";
import { useChatCompletion } from "@/hooks/useChatCompletion";
import { extractErrorMessage } from "@/utils/errorUtils";
import { formatLocalizedCost } from "@/utils/formatters";
import {
  formatDecisionReason,
  formatExclusionReason,
  formatSelectedRoute,
  formatSelectionBasis,
} from "@/utils/routingExplanation";
import type {
  ChatCompletionResponse,
  CodexRouteTelemetry,
  RoutingStrategy,
} from "@/types";

export function QuickTestPanel({
  latestCodexRoute,
}: {
  latestCodexRoute?: CodexRouteTelemetry | null;
}) {
  const { t, i18n } = useTranslation();
  const [prompt, setPrompt] = useState("");
  const [strategy, setStrategy] = useState<RoutingStrategy>("auto");
  const { mutate, isPending, data, error } = useChatCompletion();

  const handleSend = () => {
    if (!prompt.trim() || isPending) return;
    mutate({ prompt: prompt.trim(), strategy });
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      handleSend();
    }
  };

  return (
    <section className="dashboard-panel route-workbench">
      <div className="panel-heading">
        <div>
          <h2>{t("dashboard.quickTest")}</h2>
          <p>{t("dashboard.sendRealRequest")}</p>
        </div>
        {(data?.routing || latestCodexRoute) && (
          <span className="decision-time">
            {data?.routing
              ? `${data.routing.decision_time_ms ?? 0}ms`
              : `${latestCodexRoute?.latency_ms ?? 0}ms`}
          </span>
        )}
      </div>

      <div className="route-workbench-grid">
        <div className="route-result-pane" aria-live="polite">
          {isPending ? (
            <DecisionSkeleton />
          ) : data ? (
            <ResultBlock data={data} t={t} language={i18n.language} />
          ) : latestCodexRoute ? (
            <CodexResult
              route={latestCodexRoute}
              t={t}
              language={i18n.language}
            />
          ) : (
            <EmptyDecision t={t} />
          )}
        </div>

        <div className="route-composer">
          <StrategySelector value={strategy} onChange={setStrategy} />
          <label className="sr-only" htmlFor="route-prompt">
            {t("dashboard.testPlaceholder")}
          </label>
          <textarea
            id="route-prompt"
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("dashboard.testPlaceholder")}
            rows={6}
            className="route-input"
          />
          {error && <ErrorBlock message={extractErrorMessage(error)} />}
          <div className="composer-actions">
            <span className="keyboard-hint">⌘ Enter</span>
            <button
              type="button"
              onClick={handleSend}
              disabled={!prompt.trim() || isPending}
              className="send-route-button"
            >
              {isPending ? (
                <Loader2 className="is-spinning" aria-hidden="true" />
              ) : (
                <ArrowUp aria-hidden="true" />
              )}
              {isPending ? t("dashboard.routing") : t("common.send")}
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

function CodexResult({
  route,
  t,
  language,
}: {
  route: CodexRouteTelemetry;
  t: (key: string) => string;
  language: string;
}) {
  const observedAt = new Intl.DateTimeFormat(
    language.startsWith("zh") ? "zh-CN" : "en",
    {
      dateStyle: "medium",
      timeStyle: "medium",
    },
  ).format(new Date(route.timestamp));
  const reason = formatCodexReason(route, language);

  return (
    <div className="route-result codex-route-result">
      <div className="route-result-hero">
        <div className="route-result-title">
          <div>
            <span>{t("dashboard.latestCodexRoute")}</span>
            <strong>{route.model}</strong>
          </div>
          <span className="native-route-badge">Codex · Responses</span>
        </div>

        <div className="route-properties">
          <RouteProperty
            label={t("dashboard.requestedModel")}
            value={route.requested_model}
          />
          <RouteProperty
            label={t("dashboard.reasoningEffort")}
            value={route.reasoning_effort}
          />
          <RouteProperty
            label={t("dashboard.speed")}
            value={t(`dashboard.speedValue.${route.speed}`)}
          />
          <RouteProperty
            label={t("dashboard.agentTier")}
            value={t(`dashboard.agentTierValue.${route.agent_tier}`)}
          />
        </div>

        <div className="selection-basis codex-selection-basis">
          <span>{t("dashboard.decisionReason")}</span>
          <strong>{reason}</strong>
        </div>

        <div className="codex-observation">
          <span
            className={`codex-outcome ${route.success ? "is-success" : "is-failed"}`}
          >
            {route.success
              ? t("dashboard.routeSucceeded")
              : t("dashboard.routeFailed")}
            {route.status_code ? ` · HTTP ${route.status_code}` : ""}
          </span>
          <span>
            {t("dashboard.observedAt")} {observedAt}
          </span>
          <span>
            {t("dashboard.observedLatency")} {route.latency_ms}ms
          </span>
        </div>

        {route.error_message && (
          <p className="codex-route-error">
            {formatCodexError(route.error_message, t)}
          </p>
        )}
      </div>
    </div>
  );
}

function formatCodexReason(route: CodexRouteTelemetry, language: string) {
  if (language.startsWith("zh")) {
    const speed = route.speed === "priority" ? "优先速度" : "标准速度";
    const tier = {
      simple: "简单",
      medium: "中等",
      reasoning: "推理",
      complex: "复杂",
    }[route.agent_tier];
    return `Dispatcher 将任务判定为${tier}层级，选择 ${route.model}，使用 ${route.reasoning_effort} 推理力度和${speed}。`;
  }

  const speed = route.speed === "priority" ? "priority" : "standard";
  return `Dispatcher classified this as ${route.agent_tier}, selected ${route.model}, and used ${route.reasoning_effort} reasoning at ${speed} speed.`;
}

function formatCodexError(errorMessage: string, t: (key: string) => string) {
  return errorMessage === "Codex credentials are not configured"
    ? t("dashboard.codexCredentialsMissing")
    : errorMessage;
}

function EmptyDecision({ t }: { t: (key: string) => string }) {
  return (
    <div className="empty-decision">
      <div className="empty-decision-icon" aria-hidden="true">
        <Route />
      </div>
      <h3>{t("dashboard.noDecisionTitle")}</h3>
      <p>{t("dashboard.noDecisionDescription")}</p>
      <div
        className="codex-matrix"
        aria-label={t("dashboard.codexNativeMatrix")}
      >
        <span>simple</span>
        <strong>gpt-5.4-mini · low</strong>
        <span>medium</span>
        <strong>gpt-5.4 · medium</strong>
        <span>reasoning</span>
        <strong>gpt-5.5 · high</strong>
        <span>complex</span>
        <strong>gpt-5.5 · xhigh</strong>
      </div>
    </div>
  );
}

function DecisionSkeleton() {
  return (
    <div className="decision-skeleton" aria-label="loading">
      <span />
      <strong />
      <div>
        <i />
        <i />
        <i />
        <i />
      </div>
      <p />
      <p />
    </div>
  );
}

function ErrorBlock({ message }: { message: string }) {
  return (
    <div className="route-error" role="alert">
      {message}
    </div>
  );
}

function ResultBlock({
  data,
  t,
  language,
}: {
  data: ChatCompletionResponse;
  t: (key: string) => string;
  language: string;
}) {
  const content = data.choices?.[0]?.message?.content ?? "";
  const routing = data.routing;
  const usage = data.usage;
  const decisionReason = routing
    ? formatDecisionReason(routing, data.model, language)
    : "";
  const selectionBasis = routing ? formatSelectionBasis(routing, language) : "";
  const selectedRoute = routing ? formatSelectedRoute(routing, data.model) : "";

  if (!routing) {
    return (
      <div className="route-result">
        <div className="assistant-response">
          {content || t("dashboard.noData")}
        </div>
      </div>
    );
  }

  return (
    <div className="route-result">
      <div className="route-result-hero">
        <div className="route-result-title">
          <div>
            <span>{t("dashboard.selectedRoute")}</span>
            <strong>{data.model ?? "—"}</strong>
          </div>
          <span className="native-route-badge">{routing.provider}</span>
        </div>

        <div className="route-properties">
          <RouteProperty
            label={t("common.provider")}
            value={routing.provider}
          />
          <RouteProperty
            label={t("common.strategy")}
            value={routing.strategy}
          />
          <RouteProperty
            label={t("dashboard.agentTier")}
            value={routing.agent_tier}
          />
          <RouteProperty
            label={t("dashboard.decisionTime")}
            value={`${routing.decision_time_ms ?? 0}ms`}
          />
        </div>

        <div className="selection-basis">
          <span>{t("dashboard.selectionBasis")}</span>
          <strong>{selectionBasis}</strong>
          <code>{selectedRoute}</code>
        </div>

        {decisionReason && <p className="decision-reason">{decisionReason}</p>}
      </div>

      {routing.fallback_chain?.some(
        (attempt) => attempt.status === "failed",
      ) && (
        <ResultSection
          title={t("dashboard.fallbackChain")}
          detail={t("dashboard.connectionFallback")}
        >
          <div className="fallback-list">
            {routing.fallback_chain.map((attempt, index) => (
              <div
                key={`${attempt.provider_id}/${attempt.model_id}/${index}`}
                className="fallback-row"
              >
                <span>{String(index + 1).padStart(2, "0")}</span>
                <div>
                  <code>
                    {attempt.provider_id} / {attempt.model_id}
                  </code>
                  {attempt.error && (
                    <p title={attempt.error}>{attempt.error}</p>
                  )}
                </div>
                <strong className={`is-${attempt.status}`}>
                  {t(`dashboard.attemptStatus.${attempt.status}`)}
                </strong>
              </div>
            ))}
          </div>
        </ResultSection>
      )}

      {routing.top_candidates?.length ? (
        <ResultSection
          title={t("dashboard.topCandidates")}
          detail={t("dashboard.rankedByFit")}
        >
          <div className="table-scroll">
            <table className="candidate-table">
              <thead>
                <tr>
                  <th>{t("common.model")}</th>
                  <th>{t("common.provider")}</th>
                  <th>{t("dashboard.totalScore")}</th>
                  <th>{t("dashboard.estimatedCostPer1k")}</th>
                </tr>
              </thead>
              <tbody>
                {routing.top_candidates.map((candidate, index) => (
                  <tr
                    key={`${candidate.provider}/${candidate.model}`}
                    className={index === 0 ? "is-selected" : ""}
                  >
                    <td>{candidate.model}</td>
                    <td>{candidate.provider}</td>
                    <td>{candidate.total_score.toFixed(3)}</td>
                    <td>
                      {formatLocalizedCost(
                        candidate.estimated_cost_per_1k,
                        language,
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </ResultSection>
      ) : null}

      {routing.excluded_candidates?.length ? (
        <ResultSection
          title={t("dashboard.excludedCandidates")}
          detail={`${routing.excluded_candidates.length}`}
        >
          <div className="excluded-list">
            {routing.excluded_candidates.slice(0, 6).map((candidate) => (
              <div
                key={`${candidate.provider_id}/${candidate.model_id ?? "provider"}/${candidate.reason}`}
              >
                <code>
                  {candidate.provider_id}
                  {candidate.model_id ? ` / ${candidate.model_id}` : ""}
                </code>
                <span>{formatExclusionReason(candidate, language)}</span>
              </div>
            ))}
          </div>
        </ResultSection>
      ) : null}

      {content && (
        <ResultSection title={t("dashboard.response")}>
          <div className="assistant-response">{content}</div>
        </ResultSection>
      )}

      {usage && (
        <div className="usage-row">
          <span>
            {t("dashboard.promptTokens")} <strong>{usage.prompt_tokens}</strong>
          </span>
          <span>
            {t("dashboard.completionTokens")}{" "}
            <strong>{usage.completion_tokens}</strong>
          </span>
          <span>
            {t("dashboard.totalTokensLabel")}{" "}
            <strong>{usage.total_tokens}</strong>
          </span>
        </div>
      )}
    </div>
  );
}

function RouteProperty({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ResultSection({
  title,
  detail,
  children,
}: {
  title: string;
  detail?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="result-section">
      <div className="result-section-heading">
        <h3>{title}</h3>
        {detail && <span>{detail}</span>}
      </div>
      {children}
    </section>
  );
}
