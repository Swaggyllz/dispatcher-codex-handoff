import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowRight,
  Check,
  Clipboard,
  Loader2,
  RefreshCw,
  Route,
  ShieldCheck,
} from "lucide-react";
import { useHandoffContinuation } from "@/hooks/useHandoffContinuation";
import { extractErrorMessage } from "@/utils/errorUtils";
import type {
  HandoffPackageTelemetry,
  ProviderInfo,
  TelemetryStats,
} from "@/types";

interface SimpleDashboardProps {
  telemetry: TelemetryStats | undefined;
  providers: ProviderInfo[];
  isLoading: boolean;
  onRefresh: () => void;
  onShowAdvanced: () => void;
}

export function SimpleDashboard({
  telemetry,
  providers,
  isLoading,
  onRefresh,
  onShowAdvanced,
}: SimpleDashboardProps) {
  const { t, i18n } = useTranslation();
  const [copied, setCopied] = useState(false);
  const latestRoute = telemetry?.latest_codex_route ?? null;
  const latestHandoff = telemetry?.latest_handoff ?? null;
  const activeHandoff =
    latestHandoff &&
    (!latestRoute ||
      !latestRoute.success ||
      new Date(latestHandoff.created_at).getTime() >=
        new Date(latestRoute.timestamp).getTime())
      ? latestHandoff
      : null;
  const continuation = useHandoffContinuation();

  const status = useMemo(() => {
    if (activeHandoff) {
      return {
        tone: "warning",
        title: t("dashboard.simpleNeedsHandoff"),
        detail: activeHandoff.execution_state.next_recommended_step,
      };
    }

    if (latestRoute?.success) {
      return {
        tone: "success",
        title: t("dashboard.simpleCodexHealthy"),
        detail: `${latestRoute.model} · ${latestRoute.latency_ms}ms`,
      };
    }

    if (latestRoute && !latestRoute.success) {
      return {
        tone: "danger",
        title: t("dashboard.simpleRouteNeedsReview"),
        detail:
          latestRoute.error_message ??
          (latestRoute.status_code
            ? `HTTP ${latestRoute.status_code}`
            : t("dashboard.notAvailable")),
      };
    }

    return {
      tone: "idle",
      title: t("dashboard.simpleWaiting"),
      detail: t("dashboard.simpleWaitingDetail"),
    };
  }, [activeHandoff, latestRoute, t]);

  const activeProviders = providers.filter(
    (provider) => provider.health.status === "healthy",
  ).length;
  const hasProviderObservations = providers.some(
    (provider) => provider.health.status !== "unknown",
  );
  const providerDetail =
    providers.length === 0
      ? t("dashboard.noProviders")
      : !hasProviderObservations
        ? t("dashboard.simpleProviderNoSamples")
        : activeProviders === providers.length
          ? t("dashboard.simpleProviderAllHealthy")
          : activeProviders > 0
            ? t("dashboard.simpleProviderSomeNeedReview")
            : t("dashboard.simpleProviderAllNeedReview");
  const formattedTime = new Intl.DateTimeFormat(
    i18n.language.startsWith("zh") ? "zh-CN" : "en",
    {
      timeStyle: "medium",
    },
  ).format(new Date());

  const handleCopyPrompt = async () => {
    if (!activeHandoff) return;
    try {
      await navigator.clipboard.writeText(activeHandoff.continuation_prompt);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  };

  const handleContinue = () => {
    if (!activeHandoff || continuation.isPending) return;
    continuation.mutate({
      prompt: activeHandoff.continuation_prompt,
      packageId: activeHandoff.package_id,
    });
  };

  return (
    <section
      className="simple-dashboard"
      aria-label={t("dashboard.simpleMode")}
    >
      <div className={`simple-app-shell is-${status.tone}`}>
        <div className="simple-app-topbar">
          <div className="simple-window-controls" aria-hidden="true">
            <span />
            <span />
            <span />
          </div>
          <div className="simple-app-title">
            <strong>Dispatcher</strong>
            <span>{t("dashboard.simpleMode")}</span>
          </div>
          <button
            type="button"
            className="simple-topbar-button"
            onClick={onShowAdvanced}
          >
            <Route aria-hidden="true" />
            <span>{t("dashboard.professionalMode")}</span>
          </button>
        </div>

        <div className="simple-hero">
          <div className="simple-hero-copy">
            <span>{t("dashboard.simpleOverview")}</span>
            <h2>{status.title}</h2>
            <p>{isLoading ? t("common.connecting") : status.detail}</p>
            <div className="simple-primary-actions">
              <button
                type="button"
                className="simple-primary-command"
                onClick={activeHandoff ? handleContinue : onShowAdvanced}
                disabled={Boolean(activeHandoff && continuation.isPending)}
              >
                <span className="simple-command-icon">
                  {activeHandoff ? (
                    continuation.isPending ? (
                      <Loader2 className="is-spinning" />
                    ) : (
                      <ArrowRight />
                    )
                  ) : (
                    <Route />
                  )}
                </span>
                <span>
                  <strong>
                    {activeHandoff
                      ? continuation.isPending
                        ? t("dashboard.continuingHandoff")
                        : t("dashboard.continueHandoff")
                      : t("dashboard.simpleRunCheck")}
                  </strong>
                  <small>
                    {activeHandoff
                      ? t("dashboard.simpleFallbackDetail")
                      : t("dashboard.simpleRunCheckDetail")}
                  </small>
                </span>
              </button>
              <button
                type="button"
                className="simple-secondary-command"
                onClick={onRefresh}
              >
                <RefreshCw aria-hidden="true" />
                <span>{t("dashboard.simpleRefresh")}</span>
              </button>
            </div>
          </div>

          <div className="simple-status-panel">
            <div className="simple-status-orb" aria-hidden="true">
              <ShieldCheck />
            </div>
            <div className="simple-status-readout">
              <span>{t("dashboard.simpleLatestModel")}</span>
              <strong>
                {latestRoute?.model ?? latestHandoffModel(activeHandoff)}
              </strong>
              <small>
                {latestRoute?.reasoning_effort ?? t("dashboard.notAvailable")}
              </small>
            </div>
          </div>
        </div>

        <div className="simple-action-grid">
          <SimpleActionButton
            icon={copied ? <Check /> : <Clipboard />}
            label={
              copied
                ? t("dashboard.handoffPromptCopied")
                : t("dashboard.copyHandoffPrompt")
            }
            detail={
              activeHandoff
                ? activeHandoff.package_id
                : t("dashboard.simpleNoHandoffYet")
            }
            onClick={handleCopyPrompt}
            disabled={!activeHandoff}
          />
          <SimpleActionButton
            icon={
              continuation.isPending ? (
                <Loader2 className="is-spinning" />
              ) : (
                <ArrowRight />
              )
            }
            label={
              continuation.isPending
                ? t("dashboard.continuingHandoff")
                : t("dashboard.continueHandoff")
            }
            detail={t("dashboard.simpleFallbackDetail")}
            onClick={handleContinue}
            disabled={!activeHandoff || continuation.isPending}
            isPrimary={Boolean(activeHandoff)}
          />
          <SimpleActionButton
            icon={<Route />}
            label={t("dashboard.simpleRunCheck")}
            detail={t("dashboard.simpleRunCheckDetail")}
            onClick={onShowAdvanced}
          />
          <SimpleActionButton
            icon={<RefreshCw />}
            label={t("dashboard.simpleRefresh")}
            detail={t("dashboard.simpleRefreshDetail")}
            onClick={onRefresh}
          />
        </div>

        <div className="simple-signal-row">
          <SimpleSignal
            label={t("dashboard.simpleProviders")}
            value={`${activeProviders}/${providers.length || 0}`}
            detail={providerDetail}
          />
          <SimpleSignal
            label={t("dashboard.simpleLatestModel")}
            value={latestRoute?.model ?? latestHandoffModel(activeHandoff)}
            detail={
              latestRoute?.reasoning_effort ?? t("dashboard.notAvailable")
            }
          />
          <SimpleSignal
            label={t("dashboard.simpleQuota")}
            value={formatHeadroom(
              telemetry?.latest_quota_event?.normalized_headroom,
            )}
            detail={
              telemetry?.latest_quota_event?.source ??
              t("dashboard.notAvailable")
            }
          />
          <div className="simple-updated-chip">
            <span>{t("dashboard.simpleLastUpdated")}</span>
            <strong>{formattedTime}</strong>
          </div>
        </div>
      </div>

      {(continuation.data || continuation.error) && (
        <div className="simple-result-panel">
          <span>
            {continuation.error
              ? t("dashboard.fallbackContinuationFailed")
              : t("dashboard.fallbackContinuation")}
          </span>
          <p>
            {continuation.error
              ? extractErrorMessage(continuation.error)
              : extractResponsesText(continuation.data) ||
                t("dashboard.primaryReviewReady")}
          </p>
        </div>
      )}
    </section>
  );
}

function SimpleActionButton({
  icon,
  label,
  detail,
  onClick,
  disabled = false,
  isPrimary = false,
}: {
  icon: ReactNode;
  label: string;
  detail: string;
  onClick: () => void;
  disabled?: boolean;
  isPrimary?: boolean;
}) {
  return (
    <button
      type="button"
      className="simple-action-button"
      onClick={onClick}
      disabled={disabled}
      data-primary={isPrimary}
    >
      <span className="simple-action-icon">{icon}</span>
      <span>
        <strong>{label}</strong>
        <small>{detail}</small>
      </span>
    </button>
  );
}

function SimpleSignal({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="simple-signal">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}

function latestHandoffModel(handoff: HandoffPackageTelemetry | null) {
  return handoff?.routing_context.selected_model ?? "—";
}

function formatHeadroom(headroom: number | null | undefined) {
  return headroom === null || headroom === undefined
    ? "—"
    : `${(headroom * 100).toFixed(1)}%`;
}

function extractResponsesText(
  response: ReturnType<typeof useHandoffContinuation>["data"],
) {
  return (
    response?.output
      ?.flatMap((item) => item.content ?? [])
      .map((part) => part.text)
      .filter((text): text is string => Boolean(text?.trim()))
      .join("\n\n") ?? ""
  );
}
