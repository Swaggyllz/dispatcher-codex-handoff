import { useState } from "react";
import { CheckCircle2, SlidersHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  EffectivePolicyWeights,
  PolicySaveResponse,
  RoutingPolicy,
  TierPolicy,
} from "@/types";
import { PolicyEditorDialog } from "@/components/PolicyEditorDialog";

export function PolicyPanel({
  policy,
  isLoading,
}: {
  policy: RoutingPolicy | undefined;
  isLoading: boolean;
}) {
  const { t } = useTranslation();
  const [saveResult, setSaveResult] = useState<PolicySaveResponse | null>(null);

  return (
    <section className="dashboard-panel policy-panel">
      <div className="panel-heading">
        <div>
          <h2>{t("dashboard.routingPolicy")}</h2>
          <p>{t("dashboard.effectiveRuntimeConfig")}</p>
        </div>
        <div className="policy-heading-actions">
          {policy && (
            <PolicyEditorDialog policy={policy} onSaved={setSaveResult} />
          )}
          <SlidersHorizontal aria-hidden="true" />
        </div>
      </div>

      {isLoading ? (
        <PolicySkeleton />
      ) : policy ? (
        <>
          {saveResult && (
            <div className="policy-save-notice" role="status">
              <CheckCircle2 aria-hidden="true" />
              <div>
                <strong>{t("dashboard.policySaved")}</strong>
                <span>{t("dashboard.restartRequired")}</span>
                <code>{saveResult.config_path}</code>
              </div>
            </div>
          )}
          <div className="policy-governance">
            <PolicyFact
              label={t("dashboard.defaultStrategy")}
              value={t(`dashboard.strategyValue.${policy.default_strategy}`)}
            />
            <PolicyFact
              label={t("dashboard.fallback")}
              value={
                policy.fallback_enabled
                  ? t("dashboard.enabled")
                  : t("dashboard.disabled")
              }
            />
            <PolicyFact
              label={t("dashboard.circuitThreshold")}
              value={String(policy.circuit_breaker_threshold)}
            />
            <PolicyFact
              label={t("dashboard.recoveryWindow")}
              value={`${policy.circuit_breaker_timeout_secs}s`}
            />
          </div>

          <div className="policy-layout">
            <section className="policy-block">
              <div className="policy-block-heading">
                <div>
                  <h3>{t("dashboard.strategyWeights")}</h3>
                  <p>{t("dashboard.strategyWeightsDescription")}</p>
                </div>
                <WeightLegend />
              </div>
              <div className="strategy-policy-list">
                {policy.strategies.map((strategy) => (
                  <div className="strategy-policy-row" key={strategy.strategy}>
                    <div>
                      <strong>
                        {t(`dashboard.strategyValue.${strategy.strategy}`)}
                      </strong>
                      <span>
                        {strategy.overridden
                          ? t("dashboard.configured")
                          : t("dashboard.defaultValue")}
                      </span>
                    </div>
                    <WeightCells weights={strategy.weights} compact />
                  </div>
                ))}
              </div>
            </section>

            <section className="policy-block">
              <div className="policy-block-heading">
                <div>
                  <h3>{t("dashboard.tierOverrides")}</h3>
                  <p>{t("dashboard.tierOverridesDescription")}</p>
                </div>
                <span className="policy-override-count">
                  {policy.tiers.filter((tier) => tier.overridden).length}{" "}
                  {t("dashboard.overrides")}
                </span>
              </div>
              <div className="tier-policy-list">
                {policy.tiers.map((tier) => (
                  <TierPolicyRow key={tier.tier} tier={tier} />
                ))}
              </div>
            </section>
          </div>
        </>
      ) : (
        <div className="panel-empty">{t("dashboard.policyUnavailable")}</div>
      )}
    </section>
  );
}

function PolicyFact({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function TierPolicyRow({ tier }: { tier: TierPolicy }) {
  const { t } = useTranslation();
  const hasKeywords =
    tier.preferred_model_keywords.length > 0 ||
    tier.avoided_model_keywords.length > 0;

  return (
    <div className="tier-policy-row">
      <div className="tier-policy-label">
        <strong>{t(`dashboard.agentTierValue.${tier.tier}`)}</strong>
        <span className={tier.overridden ? "is-overridden" : ""}>
          {tier.overridden
            ? t("dashboard.configured")
            : t("dashboard.inheritsAuto")}
        </span>
      </div>
      <WeightCells weights={tier.weights} />
      {hasKeywords && (
        <div className="policy-keywords">
          {tier.preferred_model_keywords.map((keyword) => (
            <code className="is-preferred" key={`preferred-${keyword}`}>
              + {keyword}
            </code>
          ))}
          {tier.avoided_model_keywords.map((keyword) => (
            <code className="is-avoided" key={`avoided-${keyword}`}>
              − {keyword}
            </code>
          ))}
        </div>
      )}
    </div>
  );
}

function WeightCells({
  weights,
  compact = false,
}: {
  weights: EffectivePolicyWeights;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const entries = [
    ["quality", weights.quality],
    ["cost", weights.cost],
    ["latency", weights.latency],
    ["availability", weights.availability],
  ] as const;

  return (
    <div className={`policy-weight-cells ${compact ? "is-compact" : ""}`}>
      {entries.map(([key, weight]) => (
        <div className={weight.overridden ? "is-overridden" : ""} key={key}>
          <span>{t(`dashboard.weight.${key}`)}</span>
          <strong>{Math.round(weight.value * 100)}%</strong>
          <i style={{ "--weight": weight.value } as React.CSSProperties} />
        </div>
      ))}
    </div>
  );
}

function WeightLegend() {
  const { t } = useTranslation();
  return (
    <span className="policy-legend">
      <i />
      {t("dashboard.effectiveWeight")}
    </span>
  );
}

function PolicySkeleton() {
  return (
    <div className="policy-skeleton" aria-label="loading">
      {Array.from({ length: 8 }).map((_, index) => (
        <span key={index} />
      ))}
    </div>
  );
}
