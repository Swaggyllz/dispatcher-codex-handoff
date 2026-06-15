import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { PanelLeft, RefreshCw, Route } from "lucide-react";
import { fetchHealth } from "@/lib/api/dashboard";
import { Button } from "@/components/ui/button";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";

interface HeaderProps {
  onRefresh: () => void;
  onToggleProviders: () => void;
}

export function Header({ onRefresh, onToggleProviders }: HeaderProps) {
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
