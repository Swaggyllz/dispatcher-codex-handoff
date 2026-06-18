import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, ArrowUp, Check, Copy, Loader2, Route } from "lucide-react";
import { StrategySelector } from "@/components/StrategySelector";
import { useChatCompletion } from "@/hooks/useChatCompletion";
import { useHandoffContinuation } from "@/hooks/useHandoffContinuation";
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
  HandoffContinuationTelemetry,
  HandoffPackageTelemetry,
  ProviderContinuationResponse,
  QuotaEventTelemetry,
  RoutingStrategy,
} from "@/types";

export function QuickTestPanel({
  latestCodexRoute,
  latestQuotaEvent,
  latestHandoff,
  latestHandoffContinuation,
}: {
  latestCodexRoute?: CodexRouteTelemetry | null;
  latestQuotaEvent?: QuotaEventTelemetry | null;
  latestHandoff?: HandoffPackageTelemetry | null;
  latestHandoffContinuation?: HandoffContinuationTelemetry | null;
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
              latestQuotaEvent={latestQuotaEvent}
              latestHandoff={latestHandoff}
              latestHandoffContinuation={latestHandoffContinuation}
              t={t}
              language={i18n.language}
            />
          ) : latestHandoff ? (
            <HandoffResult
              handoff={latestHandoff}
              latestHandoffContinuation={latestHandoffContinuation}
              t={t}
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
  latestQuotaEvent,
  latestHandoff,
  latestHandoffContinuation,
  t,
  language,
}: {
  route: CodexRouteTelemetry;
  latestQuotaEvent?: QuotaEventTelemetry | null;
  latestHandoff?: HandoffPackageTelemetry | null;
  latestHandoffContinuation?: HandoffContinuationTelemetry | null;
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

        {latestQuotaEvent && (
          <QuotaSignalResult quota={latestQuotaEvent} t={t} />
        )}

        {latestHandoff && (
          <HandoffResult
            handoff={latestHandoff}
            latestHandoffContinuation={latestHandoffContinuation}
            t={t}
            isNested
          />
        )}
      </div>
    </div>
  );
}

function QuotaSignalResult({
  quota,
  t,
}: {
  quota: QuotaEventTelemetry;
  t: (key: string) => string;
}) {
  const observedHeadroom =
    quota.normalized_headroom === null
      ? t("dashboard.notAvailable")
      : `${(quota.normalized_headroom * 100).toFixed(1)}%`;

  return (
    <ResultSection title={t("dashboard.quotaSignal")}>
      <div className="route-properties">
        <RouteProperty
          label={t("dashboard.observedHeadroom")}
          value={observedHeadroom}
        />
        <RouteProperty
          label={t("dashboard.quotaSource")}
          value={formatHandoffValue(quota.source)}
        />
        <RouteProperty
          label={t("dashboard.quotaModel")}
          value={`${quota.provider_id} / ${quota.model_id}`}
        />
        {quota.retry_after_secs !== null && (
          <RouteProperty
            label={t("dashboard.retryAfter")}
            value={`${quota.retry_after_secs}s`}
          />
        )}
      </div>
    </ResultSection>
  );
}

function HandoffResult({
  handoff,
  latestHandoffContinuation,
  t,
  isNested = false,
}: {
  handoff: HandoffPackageTelemetry;
  latestHandoffContinuation?: HandoffContinuationTelemetry | null;
  t: (key: string) => string;
  isNested?: boolean;
}) {
  const [copied, setCopied] = useState(false);
  const [reviewCopied, setReviewCopied] = useState(false);
  const continuation = useHandoffContinuation();
  const persistedContinuation =
    latestHandoffContinuation?.package_id === handoff.package_id
      ? latestHandoffContinuation
      : null;

  const handleCopyPrompt = async () => {
    try {
      await navigator.clipboard.writeText(handoff.continuation_prompt);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  };

  const handleContinue = () => {
    if (continuation.isPending) return;
    continuation.mutate({
      prompt: handoff.continuation_prompt,
      packageId: handoff.package_id,
    });
  };

  const handleCopyReviewPrompt = async () => {
    if (!persistedContinuation?.review_prompt) return;
    try {
      await navigator.clipboard.writeText(persistedContinuation.review_prompt);
      setReviewCopied(true);
      window.setTimeout(() => setReviewCopied(false), 1800);
    } catch {
      setReviewCopied(false);
    }
  };

  const content = (
    <>
      <div className="route-result-title">
        <div>
          <span>{t("dashboard.latestHandoff")}</span>
          <strong>{handoff.routing_context.selected_model}</strong>
        </div>
        <span className="codex-outcome is-failed">
          {t("dashboard.emergencyReconstruction")}
        </span>
      </div>

      <div className="route-properties">
        <RouteProperty
          label={t("dashboard.handoffTrigger")}
          value={formatHandoffValue(handoff.trigger)}
        />
        <RouteProperty
          label={t("dashboard.handoffConfidence")}
          value={formatHandoffValue(handoff.confidence)}
        />
        <RouteProperty
          label={t("dashboard.executionMode")}
          value={formatHandoffValue(handoff.execution_state.mode)}
        />
        <RouteProperty
          label={t("dashboard.selectedModel")}
          value={handoff.routing_context.selected_model}
        />
      </div>

      <div className="selection-basis codex-selection-basis">
        <span>{t("dashboard.nextRecommendedStep")}</span>
        <strong>{handoff.execution_state.next_recommended_step}</strong>
      </div>

      <div className="selection-basis codex-selection-basis">
        <span>{t("dashboard.latestUserRequest")}</span>
        <strong>{handoff.latest_user_request}</strong>
      </div>

      <div className="handoff-actions">
        <button
          type="button"
          className="handoff-copy-button"
          onClick={handleCopyPrompt}
        >
          {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
          {copied
            ? t("dashboard.handoffPromptCopied")
            : t("dashboard.copyHandoffPrompt")}
        </button>
        <button
          type="button"
          className="handoff-copy-button"
          onClick={handleContinue}
          disabled={continuation.isPending}
        >
          {continuation.isPending ? (
            <Loader2 className="is-spinning" aria-hidden="true" />
          ) : (
            <ArrowRight aria-hidden="true" />
          )}
          {continuation.isPending
            ? t("dashboard.continuingHandoff")
            : t("dashboard.continueHandoff")}
        </button>
      </div>

      {continuation.error && (
        <ErrorBlock message={extractErrorMessage(continuation.error)} />
      )}

      {continuation.data && (
        <ResultSection title={t("dashboard.fallbackContinuation")}>
          <div className="route-properties">
            <RouteProperty
              label={t("dashboard.fallbackProvider")}
              value={
                continuation.data.dispatcher_provider ??
                t("dashboard.notAvailable")
              }
            />
            <RouteProperty
              label={t("dashboard.fallbackModel")}
              value={
                continuation.data.dispatcher_model ??
                continuation.data.model ??
                t("dashboard.notAvailable")
              }
            />
          </div>
          <div className="assistant-response">
            {extractResponsesText(continuation.data) || t("dashboard.noData")}
          </div>
        </ResultSection>
      )}

      {persistedContinuation && (
        <PersistedContinuationResult
          continuation={persistedContinuation}
          onCopyReviewPrompt={handleCopyReviewPrompt}
          reviewCopied={reviewCopied}
          t={t}
        />
      )}
    </>
  );

  if (isNested) {
    return (
      <ResultSection title={t("dashboard.handoffStatus")}>
        {content}
      </ResultSection>
    );
  }

  return (
    <div className="route-result codex-route-result">
      <div className="route-result-hero">{content}</div>
    </div>
  );
}

function formatHandoffValue(value: string) {
  return value.replace(/_/g, " ");
}

function PersistedContinuationResult({
  continuation,
  onCopyReviewPrompt,
  reviewCopied,
  t,
}: {
  continuation: HandoffContinuationTelemetry;
  onCopyReviewPrompt: () => void;
  reviewCopied: boolean;
  t: (key: string) => string;
}) {
  return (
    <ResultSection title={t("dashboard.savedFallbackContinuation")}>
      <div className="route-properties">
        <RouteProperty
          label={t("dashboard.fallbackProvider")}
          value={continuation.provider_id}
        />
        <RouteProperty
          label={t("dashboard.fallbackModel")}
          value={continuation.model_id}
        />
        <RouteProperty
          label={t("dashboard.handoffStatus")}
          value={
            continuation.success
              ? t("dashboard.routeSucceeded")
              : t("dashboard.routeFailed")
          }
        />
        <RouteProperty
          label={t("dashboard.continuationSource")}
          value={formatHandoffValue(continuation.source)}
        />
        <RouteProperty
          label={t("dashboard.continuationStatus")}
          value={formatHandoffValue(continuation.status)}
        />
        <RouteProperty
          label={t("dashboard.observedLatency")}
          value={`${continuation.latency_ms}ms`}
        />
      </div>

      <p className="review-status">
        {continuation.success
          ? t("dashboard.primaryReviewReady")
          : t("dashboard.fallbackContinuationFailed")}
      </p>

      <div className="assistant-response">
        {continuation.success
          ? continuation.response_text || t("dashboard.noData")
          : continuation.error_message || t("dashboard.noData")}
      </div>

      {continuation.success && (
        <div className="handoff-actions">
          <button
            type="button"
            className="handoff-copy-button"
            onClick={onCopyReviewPrompt}
          >
            {reviewCopied ? (
              <Check aria-hidden="true" />
            ) : (
              <Copy aria-hidden="true" />
            )}
            {reviewCopied
              ? t("dashboard.reviewPromptCopied")
              : t("dashboard.copyReviewPrompt")}
          </button>
        </div>
      )}
    </ResultSection>
  );
}

function extractResponsesText(response: ProviderContinuationResponse) {
  return (
    response.output
      ?.flatMap((item) => item.content ?? [])
      .map((part) => part.text)
      .filter((text): text is string => Boolean(text?.trim()))
      .join("\n\n") ?? ""
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
