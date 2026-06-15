import { useTranslation } from "react-i18next";
import type { RoutingStrategy } from "@/types";

interface StrategySelectorProps {
  value: RoutingStrategy;
  onChange: (strategy: RoutingStrategy) => void;
}

export function StrategySelector({ value, onChange }: StrategySelectorProps) {
  const { t } = useTranslation();

  const labels: Record<RoutingStrategy, string> = {
    auto: t("dashboard.strategyAuto"),
    save: t("dashboard.strategySave"),
    fast: t("dashboard.strategyFast"),
  };

  return (
    <div
      className="strategy-selector"
      role="group"
      aria-label={t("common.strategy")}
    >
      {(["auto", "save", "fast"] as RoutingStrategy[]).map((s) => (
        <button
          type="button"
          key={s}
          onClick={() => onChange(s)}
          className={
            value === s ? "strategy-option is-selected" : "strategy-option"
          }
          aria-pressed={value === s}
        >
          {labels[s]}
        </button>
      ))}
    </div>
  );
}
