import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { Header } from "@/components/Header";
import { ProviderCard } from "@/components/ProviderCard";
import { StatsPanel } from "@/components/StatsPanel";
import { ProviderStatsTable } from "@/components/ProviderStatsTable";
import { CostOverviewPanel } from "@/components/CostOverviewPanel";
import { PolicyPanel } from "@/components/PolicyPanel";
import { QuickTestPanel } from "@/components/QuickTestPanel";
import { StatusFooter } from "@/components/StatusFooter";
import { useProviders } from "@/hooks/useProviders";
import { usePolicy } from "@/hooks/usePolicy";
import { useTelemetry } from "@/hooks/useTelemetry";

export default function App() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [providersOpen, setProvidersOpen] = useState(false);
  const { data: providersData, isLoading: providersLoading } = useProviders();
  const { data: telemetry, isLoading: telemetryLoading } = useTelemetry();
  const { data: policy, isLoading: policyLoading } = usePolicy();

  const handleRefresh = () => {
    queryClient.invalidateQueries({ queryKey: ["providers"] });
    queryClient.invalidateQueries({ queryKey: ["telemetry"] });
    queryClient.invalidateQueries({ queryKey: ["health"] });
    queryClient.invalidateQueries({ queryKey: ["policy"] });
  };

  const providers = providersData?.providers ?? [];

  return (
    <div className="dashboard-app">
      <Header
        onRefresh={handleRefresh}
        onToggleProviders={() => setProvidersOpen((open) => !open)}
      />

      <div className="dashboard-shell">
        <aside
          className="provider-sidebar"
          data-mobile-open={providersOpen}
          aria-label={t("dashboard.providers")}
        >
          <div className="sidebar-heading">
            <span>{t("dashboard.providerHealth")}</span>
            <span>
              {providers.length} {t("dashboard.active")}
            </span>
          </div>
          {providersLoading ? (
            <div className="provider-list">
              {Array.from({ length: 3 }).map((_, i) => (
                <div key={i} className="provider-skeleton">
                  <div />
                  <span />
                </div>
              ))}
            </div>
          ) : providers.length === 0 ? (
            <div className="sidebar-empty">{t("dashboard.noProviders")}</div>
          ) : (
            <div className="provider-list">
              {providers.map((p) => (
                <ProviderCard key={p.id} provider={p} />
              ))}
            </div>
          )}

          <StatusFooter />
        </aside>

        <main className="dashboard-content">
          <div className="dashboard-content-inner">
            <StatsPanel telemetry={telemetry} isLoading={telemetryLoading} />
            <QuickTestPanel
              latestCodexRoute={telemetry?.latest_codex_route}
              latestQuotaEvent={telemetry?.latest_quota_event}
              latestHandoff={telemetry?.latest_handoff}
              latestHandoffContinuation={telemetry?.latest_handoff_continuation}
            />
            <PolicyPanel policy={policy} isLoading={policyLoading} />
            <CostOverviewPanel
              telemetry={telemetry}
              isLoading={telemetryLoading}
            />
            <ProviderStatsTable
              telemetry={telemetry}
              isLoading={telemetryLoading}
            />
          </div>
        </main>
      </div>
    </div>
  );
}
