import { useTranslation } from "react-i18next";
import type { TelemetryStats } from "@/types";
import { formatLocalizedCost } from "@/utils/formatters";

interface StatsPanelProps {
  telemetry: TelemetryStats | undefined;
  isLoading: boolean;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function StatsPanel({ telemetry, isLoading }: StatsPanelProps) {
  const { t, i18n } = useTranslation();

  const metrics = [
    {
      label: t("dashboard.totalRequests"),
      value: telemetry ? String(telemetry.total_requests) : "—",
      note: t("dashboard.allRecordedRequests"),
    },
    {
      label: t("dashboard.successRate"),
      value: telemetry ? `${(telemetry.success_rate * 100).toFixed(1)}%` : "—",
      note: t("dashboard.completedSuccessfully"),
      tone:
        telemetry && telemetry.success_rate >= 0.95 ? "positive" : "warning",
    },
    {
      label: t("dashboard.totalTokens"),
      value: telemetry ? formatTokens(telemetry.total_tokens) : "—",
      note: t("dashboard.inputAndOutput"),
    },
    {
      label: t("dashboard.avgLatency"),
      value: telemetry ? `${telemetry.avg_latency_ms.toFixed(0)}ms` : "—",
      note: t("dashboard.observedAverage"),
      tone: "warning",
    },
    {
      label: t("dashboard.totalCost"),
      value: telemetry
        ? formatLocalizedCost(telemetry.total_cost_usd, i18n.language)
        : "—",
      note: t("dashboard.estimatedSpend"),
    },
  ];

  return (
    <section className="metric-strip" aria-label={t("dashboard.routingStats")}>
      {metrics.map((metric) => (
        <div key={metric.label} className="metric-cell">
          <span className="metric-label">{metric.label}</span>
          <strong
            className={`metric-value ${metric.tone ? `is-${metric.tone}` : ""} ${
              isLoading ? "is-loading" : ""
            }`}
          >
            {isLoading ? "…" : metric.value}
          </strong>
          <span className="metric-note">{metric.note}</span>
        </div>
      ))}
    </section>
  );
}
