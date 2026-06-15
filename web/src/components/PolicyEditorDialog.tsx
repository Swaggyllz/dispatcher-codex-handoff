import { useMemo, useState } from "react";
import {
  AlertTriangle,
  Check,
  LoaderCircle,
  Pencil,
  RotateCcw,
  Save,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { DashboardApiError } from "@/lib/api/dashboard";
import { useSavePolicy } from "@/hooks/usePolicy";
import type {
  AgentTier,
  EditableTierPolicy,
  PolicySaveResponse,
  PolicyUpdate,
  PolicyWeights,
  RoutingPolicy,
  RoutingStrategy,
} from "@/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

const STRATEGIES = ["auto", "save", "fast"] as const;
const TIERS = ["simple", "medium", "reasoning", "complex"] as const;
const WEIGHTS = ["quality", "cost", "latency", "availability"] as const;

export function PolicyEditorDialog({
  policy,
  onSaved,
}: {
  policy: RoutingPolicy;
  onSaved: (result: PolicySaveResponse) => void;
}) {
  const { t } = useTranslation();
  const mutation = useSavePolicy();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<PolicyUpdate | null>(null);

  const initial = policy.editable_policy;
  const dirty = useMemo(
    () =>
      Boolean(
        initial && draft && JSON.stringify(initial) !== JSON.stringify(draft),
      ),
    [draft, initial],
  );

  const handleOpenChange = (nextOpen: boolean) => {
    if (mutation.isPending) return;
    setOpen(nextOpen);
    if (nextOpen && initial) {
      setDraft(clonePolicy(initial));
      mutation.reset();
    }
  };

  const resetDraft = () => {
    if (initial) {
      setDraft(clonePolicy(initial));
      mutation.reset();
    }
  };

  const save = async () => {
    if (!draft || !dirty) return;
    try {
      const result = await mutation.mutateAsync(draft);
      setDraft(clonePolicy(result.saved_policy.editable_policy ?? draft));
      onSaved(result);
      setOpen(false);
    } catch {
      // The mutation state renders structured validation errors below.
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger asChild>
        <Button
          className="policy-edit-trigger"
          disabled={!policy.editable || !initial}
          size="icon"
          title={
            policy.editable
              ? t("dashboard.editPolicy")
              : t("dashboard.policyReadOnly")
          }
          variant="ghost"
        >
          <Pencil aria-hidden="true" />
          <span className="sr-only">{t("dashboard.editPolicy")}</span>
        </Button>
      </DialogTrigger>

      {draft && (
        <DialogContent className="policy-editor-dialog">
          <DialogHeader className="policy-editor-header">
            <div>
              <DialogTitle>{t("dashboard.editRoutingPolicy")}</DialogTitle>
              <DialogDescription>
                {t("dashboard.policyEditDescription")}
              </DialogDescription>
            </div>
            <DialogClose asChild>
              <Button
                disabled={mutation.isPending}
                size="icon"
                title={t("dashboard.closeEditor")}
                variant="ghost"
              >
                <X aria-hidden="true" />
              </Button>
            </DialogClose>
          </DialogHeader>

          <div className="policy-editor-body">
            <section className="policy-editor-section">
              <div className="policy-editor-section-heading">
                <div>
                  <h3>{t("dashboard.policyGovernance")}</h3>
                  <p>{t("dashboard.policyGovernanceDescription")}</p>
                </div>
                <span className={dirty ? "is-dirty" : ""}>
                  {dirty
                    ? t("dashboard.unsavedChanges")
                    : t("dashboard.noUnsavedChanges")}
                </span>
              </div>

              <div className="policy-editor-governance">
                <label>
                  <span>{t("dashboard.defaultStrategy")}</span>
                  <div className="policy-segmented-control">
                    {STRATEGIES.map((strategy) => (
                      <button
                        aria-pressed={draft.default_strategy === strategy}
                        data-active={draft.default_strategy === strategy}
                        key={strategy}
                        onClick={() =>
                          setDraft({
                            ...draft,
                            default_strategy: strategy,
                          })
                        }
                        type="button"
                      >
                        {t(`dashboard.strategyValue.${strategy}`)}
                      </button>
                    ))}
                  </div>
                </label>

                <label className="policy-switch-field">
                  <span>
                    <strong>{t("dashboard.fallback")}</strong>
                    <small>{t("dashboard.fallbackDescription")}</small>
                  </span>
                  <Switch
                    checked={draft.fallback_enabled}
                    onCheckedChange={(checked) =>
                      setDraft({ ...draft, fallback_enabled: checked })
                    }
                  />
                </label>

                <NumberField
                  label={t("dashboard.circuitThreshold")}
                  max={100}
                  min={1}
                  onChange={(value) =>
                    setDraft({
                      ...draft,
                      circuit_breaker_threshold: value,
                    })
                  }
                  value={draft.circuit_breaker_threshold}
                />
                <NumberField
                  label={t("dashboard.recoveryWindowSeconds")}
                  max={3600}
                  min={1}
                  onChange={(value) =>
                    setDraft({
                      ...draft,
                      circuit_breaker_timeout_secs: value,
                    })
                  }
                  value={draft.circuit_breaker_timeout_secs}
                />
              </div>
            </section>

            <section className="policy-editor-section">
              <div className="policy-editor-section-heading">
                <div>
                  <h3>{t("dashboard.strategyWeights")}</h3>
                  <p>{t("dashboard.weightSumHint")}</p>
                </div>
              </div>
              <div className="policy-editor-matrix">
                <div className="policy-editor-matrix-head">
                  <span />
                  {WEIGHTS.map((weight) => (
                    <span key={weight}>{t(`dashboard.weight.${weight}`)}</span>
                  ))}
                </div>
                {STRATEGIES.map((strategy) => (
                  <div className="policy-editor-matrix-row" key={strategy}>
                    <strong>{t(`dashboard.strategyValue.${strategy}`)}</strong>
                    {WEIGHTS.map((weight) => (
                      <PercentInput
                        key={weight}
                        onChange={(value) =>
                          setDraft(
                            updateStrategyWeight(
                              draft,
                              strategy,
                              weight,
                              value,
                            ),
                          )
                        }
                        value={draft.strategy_weights[strategy][weight]}
                      />
                    ))}
                  </div>
                ))}
              </div>
            </section>

            <section className="policy-editor-section">
              <div className="policy-editor-section-heading">
                <div>
                  <h3>{t("dashboard.tierOverrides")}</h3>
                  <p>{t("dashboard.tierEditDescription")}</p>
                </div>
              </div>
              <div className="policy-tier-editor-list">
                {TIERS.map((tier) => (
                  <TierEditor
                    draft={draft}
                    key={tier}
                    onChange={setDraft}
                    policy={policy}
                    tier={tier}
                  />
                ))}
              </div>
            </section>

            <PolicyEditorFeedback error={mutation.error} />
          </div>

          <DialogFooter className="policy-editor-footer">
            <div className="policy-config-path">
              <span>{t("dashboard.savesTo")}</span>
              <code>{policy.config_path}</code>
            </div>
            <Button
              disabled={!dirty || mutation.isPending}
              onClick={resetDraft}
              variant="outline"
            >
              <RotateCcw aria-hidden="true" />
              {t("dashboard.resetChanges")}
            </Button>
            <Button disabled={!dirty || mutation.isPending} onClick={save}>
              {mutation.isPending ? (
                <LoaderCircle
                  className="policy-saving-icon"
                  aria-hidden="true"
                />
              ) : mutation.isSuccess ? (
                <Check aria-hidden="true" />
              ) : (
                <Save aria-hidden="true" />
              )}
              {mutation.isPending
                ? t("dashboard.savingPolicy")
                : mutation.isSuccess && !dirty
                  ? t("dashboard.policySaved")
                  : t("dashboard.savePolicy")}
            </Button>
          </DialogFooter>
        </DialogContent>
      )}
    </Dialog>
  );
}

function TierEditor({
  draft,
  onChange,
  policy,
  tier,
}: {
  draft: PolicyUpdate;
  onChange: (policy: PolicyUpdate) => void;
  policy: RoutingPolicy;
  tier: AgentTier;
}) {
  const { t } = useTranslation();
  const override = draft.tier_policies[tier];
  const effective = policy.tiers.find((item) => item.tier === tier);

  const toggleOverride = (enabled: boolean) => {
    const tierPolicies = { ...draft.tier_policies };
    if (!enabled) {
      delete tierPolicies[tier];
    } else if (effective) {
      tierPolicies[tier] = {
        quality_weight: effective.weights.quality.value,
        cost_weight: effective.weights.cost.value,
        latency_weight: effective.weights.latency.value,
        preferred_model_keywords: [],
        avoided_model_keywords: [],
      };
    }
    onChange({ ...draft, tier_policies: tierPolicies });
  };

  return (
    <div className="policy-tier-editor">
      <div className="policy-tier-editor-head">
        <div>
          <strong>{t(`dashboard.agentTierValue.${tier}`)}</strong>
          <span>
            {override
              ? t("dashboard.customOverride")
              : t("dashboard.inheritsAuto")}
          </span>
        </div>
        <Switch checked={Boolean(override)} onCheckedChange={toggleOverride} />
      </div>

      {override && (
        <div className="policy-tier-editor-fields">
          <div className="policy-tier-weight-grid">
            {(
              [
                ["quality_weight", "quality"],
                ["cost_weight", "cost"],
                ["latency_weight", "latency"],
              ] as const
            ).map(([field, label]) => (
              <label key={field}>
                <span>{t(`dashboard.weight.${label}`)}</span>
                <PercentInput
                  onChange={(value) =>
                    onChange(updateTierWeight(draft, tier, field, value))
                  }
                  value={override[field] ?? 0}
                />
              </label>
            ))}
            <label>
              <span>{t("dashboard.weight.availability")}</span>
              <PercentInput
                disabled
                value={draft.strategy_weights.auto.availability}
              />
            </label>
          </div>
          <KeywordField
            label={t("dashboard.preferredKeywords")}
            onChange={(keywords) =>
              onChange(updateTierKeywords(draft, tier, true, keywords))
            }
            value={override.preferred_model_keywords}
          />
          <KeywordField
            label={t("dashboard.avoidedKeywords")}
            onChange={(keywords) =>
              onChange(updateTierKeywords(draft, tier, false, keywords))
            }
            value={override.avoided_model_keywords}
          />
        </div>
      )}
    </div>
  );
}

function NumberField({
  label,
  max,
  min,
  onChange,
  value,
}: {
  label: string;
  max: number;
  min: number;
  onChange: (value: number) => void;
  value: number;
}) {
  return (
    <label className="policy-number-field">
      <span>{label}</span>
      <Input
        max={max}
        min={min}
        onChange={(event) => onChange(Number(event.target.value))}
        type="number"
        value={value}
      />
    </label>
  );
}

function PercentInput({
  disabled = false,
  onChange,
  value,
}: {
  disabled?: boolean;
  onChange?: (value: number) => void;
  value: number;
}) {
  return (
    <div className="policy-percent-input">
      <Input
        disabled={disabled}
        max={100}
        min={0}
        onChange={(event) => onChange?.(Number(event.target.value) / 100)}
        step={1}
        type="number"
        value={Math.round(value * 100)}
      />
      <span>%</span>
    </div>
  );
}

function KeywordField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string[]) => void;
  value: string[];
}) {
  const { t } = useTranslation();
  return (
    <label className="policy-keyword-field">
      <span>{label}</span>
      <Input
        onChange={(event) =>
          onChange(
            event.target.value
              .split(",")
              .map((keyword) => keyword.trim())
              .filter(Boolean),
          )
        }
        placeholder={t("dashboard.keywordPlaceholder")}
        value={value.join(", ")}
      />
    </label>
  );
}

function PolicyEditorFeedback({ error }: { error: Error | null }) {
  const { t } = useTranslation();
  if (!error) return null;

  const fields = error instanceof DashboardApiError ? error.fields : [];
  return (
    <div className="policy-editor-error" role="alert">
      <AlertTriangle aria-hidden="true" />
      <div>
        <strong>{t("dashboard.policyValidationFailed")}</strong>
        <p>{error.message}</p>
        {fields.length > 0 && (
          <ul>
            {fields.map((field) => (
              <li key={`${field.field}-${field.message}`}>
                <code>{field.field}</code> {field.message}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function clonePolicy(policy: PolicyUpdate): PolicyUpdate {
  return JSON.parse(JSON.stringify(policy)) as PolicyUpdate;
}

function updateStrategyWeight(
  draft: PolicyUpdate,
  strategy: RoutingStrategy,
  weight: keyof PolicyWeights,
  value: number,
): PolicyUpdate {
  return {
    ...draft,
    strategy_weights: {
      ...draft.strategy_weights,
      [strategy]: {
        ...draft.strategy_weights[strategy],
        [weight]: value,
      },
    },
  };
}

function updateTierWeight(
  draft: PolicyUpdate,
  tier: AgentTier,
  field: "quality_weight" | "cost_weight" | "latency_weight",
  value: number,
): PolicyUpdate {
  const current = draft.tier_policies[tier];
  if (!current) return draft;
  return {
    ...draft,
    tier_policies: {
      ...draft.tier_policies,
      [tier]: { ...current, [field]: value },
    },
  };
}

function updateTierKeywords(
  draft: PolicyUpdate,
  tier: AgentTier,
  preferred: boolean,
  keywords: string[],
): PolicyUpdate {
  const current = draft.tier_policies[tier];
  if (!current) return draft;
  const field: keyof Pick<
    EditableTierPolicy,
    "preferred_model_keywords" | "avoided_model_keywords"
  > = preferred ? "preferred_model_keywords" : "avoided_model_keywords";
  return {
    ...draft,
    tier_policies: {
      ...draft.tier_policies,
      [tier]: { ...current, [field]: keywords },
    },
  };
}
