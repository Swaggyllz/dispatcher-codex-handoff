import { useTranslation } from "react-i18next";
import type { TelemetryStats } from "@/types";
import { formatLocalizedCost } from "@/utils/formatters";

interface ProviderStatsTableProps {
  telemetry: TelemetryStats | undefined;
  isLoading: boolean;
}

export function ProviderStatsTable({
  telemetry,
  isLoading,
}: ProviderStatsTableProps) {
  const { t, i18n } = useTranslation();
  const stats = telemetry?.provider_stats ?? [];
  const totalCost = telemetry?.total_cost_usd ?? 0;
  const costShare = (cost: number) =>
    totalCost > 0 ? `${((cost / totalCost) * 100).toFixed(1)}%` : "—";

  if (!isLoading && stats.length === 0) {
    return (
      <section className="dashboard-panel provider-breakdown-panel">
        <div className="panel-heading">
          <div>
            <h2>{t("dashboard.providerBreakdown")}</h2>
            <p>{t("dashboard.last24Hours")}</p>
          </div>
        </div>
        <div className="panel-empty">{t("dashboard.noData")}</div>
      </section>
    );
  }

  return (
    <section className="dashboard-panel provider-breakdown-panel">
      <div className="panel-heading">
        <div>
          <h2>{t("dashboard.providerBreakdown")}</h2>
          <p>{t("dashboard.last24Hours")}</p>
        </div>
      </div>
      <div className="table-scroll provider-table-wrap">
        <table className="provider-table">
          <thead>
            <tr>
              <th>{t("dashboard.providerModel")}</th>
              <th>{t("dashboard.totalRequests")}</th>
              <th>{t("dashboard.promptTokens")}</th>
              <th>{t("dashboard.completionTokens")}</th>
              <th>{t("dashboard.totalCost")}</th>
              <th>{t("dashboard.costShare")}</th>
              <th>{t("dashboard.avgLatency")}</th>
              <th>{t("dashboard.successRate")}</th>
            </tr>
          </thead>
          <tbody>
            {isLoading
              ? Array.from({ length: 3 }).map((_, i) => (
                  <tr key={i} className="table-skeleton">
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                    <td>
                      <span />
                    </td>
                  </tr>
                ))
              : stats.flatMap((s) => {
                  const successRate =
                    s.total_requests > 0
                      ? (s.success_count / s.total_requests) * 100
                      : 0;
                  const providerRow = (
                    <tr key={s.provider_id} className="provider-summary-row">
                      <td>
                        <strong>{s.provider_id}</strong>
                      </td>
                      <td>{s.total_requests}</td>
                      <td>{s.request_tokens}</td>
                      <td>{s.response_tokens}</td>
                      <td>
                        {formatLocalizedCost(s.total_cost_usd, i18n.language)}
                      </td>
                      <td>{costShare(s.total_cost_usd)}</td>
                      <td>{s.avg_latency_ms.toFixed(0)}ms</td>
                      <td>
                        <span
                          className="success-rate"
                          style={{
                            color:
                              successRate >= 95
                                ? "var(--success)"
                                : successRate >= 70
                                  ? "var(--warning)"
                                  : "var(--destructive)",
                          }}
                        >
                          {successRate.toFixed(1)}%
                        </span>
                      </td>
                    </tr>
                  );

                  const modelRows = (s.model_stats ?? []).map((model) => {
                    const modelSuccessRate =
                      model.total_requests > 0
                        ? (model.success_count / model.total_requests) * 100
                        : 0;
                    return (
                      <tr
                        key={`${s.provider_id}:${model.model_id}`}
                        className="provider-cost-model-row"
                      >
                        <td>
                          <span>{model.model_id}</span>
                        </td>
                        <td>{model.total_requests}</td>
                        <td>{model.request_tokens}</td>
                        <td>{model.response_tokens}</td>
                        <td>
                          {formatLocalizedCost(
                            model.total_cost_usd,
                            i18n.language,
                          )}
                        </td>
                        <td>{costShare(model.total_cost_usd)}</td>
                        <td>{model.avg_latency_ms.toFixed(0)}ms</td>
                        <td>
                          <span className="success-rate">
                            {modelSuccessRate.toFixed(1)}%
                          </span>
                        </td>
                      </tr>
                    );
                  });

                  return [providerRow, ...modelRows];
                })}
          </tbody>
        </table>
      </div>
    </section>
  );
}
