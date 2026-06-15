import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, Eye, Radio, Wrench } from "lucide-react";
import type { ProviderInfo } from "@/types";
import { formatLocalizedCost } from "@/utils/formatters";

interface ProviderCardProps {
  provider: ProviderInfo;
}

export function ProviderCard({ provider }: ProviderCardProps) {
  const { t, i18n } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const successRate =
    provider.health.success_rate === null
      ? null
      : `${(provider.health.success_rate * 100).toFixed(1)}%`;

  return (
    <div className="provider-card" data-expanded={expanded}>
      <button
        type="button"
        className="provider-card-trigger"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <span className={`provider-health-dot is-${provider.health.status}`} />
        <span className="provider-card-copy">
          <strong>{provider.name}</strong>
          <span>
            {provider.models.length} {t("common.models")}
            {provider.health.sample_count > 0 &&
              ` · ${provider.health.sample_count} ${t("common.reqs")}`}
          </span>
        </span>
        <span className="provider-card-status">
          <span className={`health-label is-${provider.health.status}`}>
            {t(`dashboard.healthStatus.${provider.health.status}`)}
          </span>
          <ChevronDown
            className={expanded ? "is-expanded" : ""}
            aria-hidden="true"
          />
        </span>
      </button>

      {expanded && (
        <div className="provider-card-details">
          <div className="provider-health-grid">
            <HealthMetric
              label={t("dashboard.successRate")}
              value={successRate ?? t("dashboard.notAvailable")}
            />
            <HealthMetric
              label={t("dashboard.observedLatency")}
              value={
                provider.health.avg_latency_ms === null
                  ? t("dashboard.notAvailable")
                  : `${provider.health.avg_latency_ms}ms`
              }
            />
            <HealthMetric
              label={t("dashboard.circuitState")}
              value={t(`dashboard.circuit.${provider.health.circuit_state}`)}
              accent={provider.health.circuit_state !== "closed"}
            />
          </div>
          <div className="provider-capabilities">
            <Capability
              enabled={provider.supports_streaming}
              icon={Radio}
              label={t("dashboard.streaming")}
            />
            <Capability
              enabled={provider.supports_vision}
              icon={Eye}
              label={t("dashboard.vision")}
            />
            <Capability
              enabled={provider.supports_tools}
              icon={Wrench}
              label={t("dashboard.tools")}
            />
          </div>
          <div className="provider-model-list">
            {provider.models.map((model) => (
              <div key={model.id} className="provider-model-row">
                <span title={model.name}>{model.name}</span>
                <span>{Math.round(model.quality_score * 100)}%</span>
                <span>{model.avg_latency_ms}ms</span>
                <span className="provider-model-price">
                  {t("dashboard.inputPriceShort")}{" "}
                  {formatLocalizedCost(model.cost.input_per_1k, i18n.language)}
                </span>
                <span className="provider-model-price">
                  {t("dashboard.outputPriceShort")}{" "}
                  {formatLocalizedCost(model.cost.output_per_1k, i18n.language)}
                </span>
                <span className="provider-model-source">
                  {model.pricing_source
                    ? t("dashboard.priceSource", {
                        source: model.pricing_source,
                      })
                    : t("dashboard.priceSourceUnknown")}
                  {model.pricing_updated_at &&
                    ` · ${t("dashboard.priceUpdatedAt", {
                      date: model.pricing_updated_at,
                    })}`}
                </span>
                <span className="provider-model-capabilities">
                  <Capability
                    enabled={model.supports_streaming}
                    icon={Radio}
                    label={t("dashboard.streaming")}
                  />
                  <Capability
                    enabled={model.supports_vision}
                    icon={Eye}
                    label={t("dashboard.vision")}
                  />
                  <Capability
                    enabled={model.supports_tools}
                    icon={Wrench}
                    label={t("dashboard.tools")}
                  />
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function HealthMetric({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: boolean;
}) {
  return (
    <div className="provider-health-metric">
      <span>{label}</span>
      <strong className={accent ? "is-warning" : ""}>{value}</strong>
    </div>
  );
}

function Capability({
  enabled,
  icon: Icon,
  label,
}: {
  enabled: boolean;
  icon: typeof Radio;
  label: string;
}) {
  return (
    <span className={enabled ? "capability is-enabled" : "capability"}>
      <Icon aria-hidden="true" />
      {label}
    </span>
  );
}
