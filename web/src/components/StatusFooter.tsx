import { useTranslation } from "react-i18next";

export function StatusFooter() {
  const { t } = useTranslation();
  const apiBase = `${window.location.origin}/v1`;

  return (
    <footer className="sidebar-footer">
      <span>{t("dashboard.apiEndpoint")}</span>
      <code>{apiBase}</code>
      <span>{t("dashboard.autoRefresh")}</span>
    </footer>
  );
}
