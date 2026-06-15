import { useTranslation } from "react-i18next";
import type {
  CostBreakdown,
  StrategyCostBreakdown,
  TelemetryStats,
  TierCostBreakdown,
} from "@/types";
import { formatLocalizedCost } from "@/utils/formatters";

interface CostOverviewPanelProps {
  telemetry: TelemetryStats | undefined;
  isLoading: boolean;
}

export function CostOverviewPanel({
  telemetry,
  isLoading,
}: CostOverviewPanelProps) {
  const { t, i18n } = useTranslation();
  const summary = telemetry?.cost_summary;

  return (
    <section className="dashboard-panel cost-overview-panel">
      <div className="panel-heading">
        <div>
          <h2>{t("dashboard.costOverview")}</h2>
          <p>{t("dashboard.costOverviewDescription")}</p>
        </div>
      </div>

      <div className="cost-overview-grid">
        <div className="cost-period-grid">
          <CostPeriod
            label={t("dashboard.todayCost")}
            value={
              summary
                ? formatLocalizedCost(summary.today_usd, i18n.language)
                : "—"
            }
            isLoading={isLoading}
          />
          <CostPeriod
            label={t("dashboard.monthCost")}
            value={
              summary
                ? formatLocalizedCost(summary.month_usd, i18n.language)
                : "—"
            }
            isLoading={isLoading}
          />
        </div>

        <CostBreakdownList
          title={t("dashboard.costByTier")}
          rows={telemetry?.cost_by_tier ?? []}
          label={(row) =>
            t(
              `dashboard.agentTierValue.${(row as TierCostBreakdown).agent_tier}`,
            )
          }
          isLoading={isLoading}
          language={i18n.language}
          requestUnit={t("common.reqs")}
        />

        <CostBreakdownList
          title={t("dashboard.costByStrategy")}
          rows={telemetry?.cost_by_strategy ?? []}
          label={(row) => {
            const strategy = (
              row as StrategyCostBreakdown
            ).routing_strategy.toLowerCase();
            return t(`dashboard.strategyValue.${strategy}`, {
              defaultValue: (row as StrategyCostBreakdown).routing_strategy,
            });
          }}
          isLoading={isLoading}
          language={i18n.language}
          requestUnit={t("common.reqs")}
        />
      </div>
    </section>
  );
}

function CostPeriod({
  label,
  value,
  isLoading,
}: {
  label: string;
  value: string;
  isLoading: boolean;
}) {
  return (
    <div className="cost-period">
      <span>{label}</span>
      <strong className={isLoading ? "is-loading" : ""}>
        {isLoading ? "..." : value}
      </strong>
    </div>
  );
}

function CostBreakdownList({
  title,
  rows,
  label,
  isLoading,
  language,
  requestUnit,
}: {
  title: string;
  rows: CostBreakdown[];
  label: (row: CostBreakdown) => string;
  isLoading: boolean;
  language: string;
  requestUnit: string;
}) {
  return (
    <div className="cost-breakdown">
      <h3>{title}</h3>
      {isLoading ? (
        <div className="cost-breakdown-empty">...</div>
      ) : rows.length === 0 ? (
        <div className="cost-breakdown-empty">—</div>
      ) : (
        rows.map((row) => (
          <div className="cost-breakdown-row" key={label(row)}>
            <div>
              <strong>{label(row)}</strong>
              <span>
                {row.total_requests} {requestUnit}
              </span>
            </div>
            <strong>{formatLocalizedCost(row.total_cost_usd, language)}</strong>
          </div>
        ))
      )}
    </div>
  );
}
