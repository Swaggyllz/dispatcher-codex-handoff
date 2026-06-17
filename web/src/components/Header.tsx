import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { LayoutDashboard, PanelLeft, RefreshCw, Route } from "lucide-react";
import { fetchHealth } from "@/lib/api/dashboard";
import { Button } from "@/components/ui/button";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";

interface HeaderProps {
  dashboardMode: "simple" | "professional";
  onDashboardModeChange: (mode: "simple" | "professional") => void;
  onRefresh: () => void;
  onToggleProviders: () => void;
}

export function Header({
  dashboardMode,
  onDashboardModeChange,
  onRefresh,
  onToggleProviders,
}: HeaderProps) {
  const { t } = useTranslation();
  const { data: health, isError } = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
    refetchInterval: 30_000,
  });

  return (
    <header className="dashboard-header">
      <div className="brand-lockup">
        <div className="brand-mark" aria-hidden="true">
          <Route />
        </div>
        <div className="brand-copy">
          <h1>Dispatcher</h1>
          <p>{t("app.subtitle")}</p>
        </div>
      </div>

      <div className="header-actions">
        <div className="mode-switch" aria-label={t("dashboard.dashboardMode")}>
          <button
            type="button"
            onClick={() => onDashboardModeChange("simple")}
            data-active={dashboardMode === "simple"}
            aria-pressed={dashboardMode === "simple"}
            aria-label={t("dashboard.simpleMode")}
          >
            <LayoutDashboard aria-hidden="true" />
            <span>{t("dashboard.simpleMode")}</span>
          </button>
          <button
            type="button"
            onClick={() => onDashboardModeChange("professional")}
            data-active={dashboardMode === "professional"}
            aria-pressed={dashboardMode === "professional"}
            aria-label={t("dashboard.professionalMode")}
          >
            <Route aria-hidden="true" />
            <span>{t("dashboard.professionalMode")}</span>
          </button>
        </div>
        {dashboardMode === "professional" && (
          <Button
            variant="ghost"
            size="icon"
            onClick={onToggleProviders}
            className="provider-menu-button"
            aria-label={t("dashboard.providers")}
            title={t("dashboard.providers")}
          >
            <PanelLeft />
          </Button>
        )}
        <LanguageSwitcher />
        <div className="service-status">
          <span
            className={`service-dot ${
              isError ? "is-error" : health ? "is-healthy" : ""
            }`}
          />
          <span>
            {health
              ? t("dashboard.localServiceHealthy")
              : t("common.connecting")}
          </span>
        </div>
        {health && <span className="version-badge">v{health.version}</span>}
        <Button
          variant="ghost"
          size="icon"
          onClick={onRefresh}
          className="header-icon-button"
          aria-label={t("common.refresh")}
          title={t("common.refresh")}
        >
          <RefreshCw />
        </Button>
      </div>
    </header>
  );
}
