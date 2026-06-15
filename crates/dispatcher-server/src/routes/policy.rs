use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use dispatcher_engine::{
    AgentTier, RoutingConfig, RoutingStrategy, StrategyWeights, TierRoutingPolicy,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct PolicyView {
    default_strategy: RoutingStrategy,
    fallback_enabled: bool,
    circuit_breaker_threshold: u32,
    circuit_breaker_timeout_secs: u64,
    strategies: Vec<StrategyPolicyView>,
    tiers: Vec<TierPolicyView>,
    editable: bool,
    config_path: Option<String>,
    editable_policy: Option<PolicyUpdate>,
}

#[derive(Debug, Serialize)]
struct StrategyPolicyView {
    strategy: String,
    weights: EffectiveWeights,
    overridden: bool,
}

#[derive(Debug, Serialize)]
struct TierPolicyView {
    tier: AgentTier,
    weights: EffectiveWeights,
    preferred_model_keywords: Vec<String>,
    avoided_model_keywords: Vec<String>,
    overridden: bool,
}

#[derive(Debug, Serialize)]
struct EffectiveWeights {
    quality: EffectiveWeight,
    cost: EffectiveWeight,
    latency: EffectiveWeight,
    availability: EffectiveWeight,
}

#[derive(Debug, Serialize)]
struct EffectiveWeight {
    value: f64,
    overridden: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyUpdate {
    default_strategy: RoutingStrategy,
    fallback_enabled: bool,
    circuit_breaker_threshold: u32,
    circuit_breaker_timeout_secs: u64,
    strategy_weights: HashMap<String, StrategyWeights>,
    tier_policies: HashMap<AgentTier, EditableTierPolicy>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EditableTierPolicy {
    quality_weight: Option<f64>,
    cost_weight: Option<f64>,
    latency_weight: Option<f64>,
    #[serde(default)]
    preferred_model_keywords: Vec<String>,
    #[serde(default)]
    avoided_model_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ValidationError {
    field: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct PolicySaveResponse {
    saved: bool,
    restart_required: bool,
    config_path: String,
    saved_policy: PolicyView,
}

impl PolicyUpdate {
    fn from_config(config: &RoutingConfig) -> Self {
        Self {
            default_strategy: config.default_strategy,
            fallback_enabled: config.fallback_enabled,
            circuit_breaker_threshold: config.circuit_breaker_threshold,
            circuit_breaker_timeout_secs: config.circuit_breaker_timeout_secs,
            strategy_weights: config.strategy_weights.clone(),
            tier_policies: config
                .tier_policies
                .iter()
                .map(|(tier, policy)| {
                    (
                        *tier,
                        EditableTierPolicy {
                            quality_weight: policy.quality_weight,
                            cost_weight: policy.cost_weight,
                            latency_weight: policy.latency_weight,
                            preferred_model_keywords: policy.preferred_model_keywords.clone(),
                            avoided_model_keywords: policy.avoided_model_keywords.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

fn effective_weights(
    base: &StrategyWeights,
    override_policy: Option<&TierRoutingPolicy>,
) -> EffectiveWeights {
    EffectiveWeights {
        quality: EffectiveWeight {
            value: override_policy
                .and_then(|policy| policy.quality_weight)
                .unwrap_or(base.quality),
            overridden: override_policy.is_some_and(|policy| policy.quality_weight.is_some()),
        },
        cost: EffectiveWeight {
            value: override_policy
                .and_then(|policy| policy.cost_weight)
                .unwrap_or(base.cost),
            overridden: override_policy.is_some_and(|policy| policy.cost_weight.is_some()),
        },
        latency: EffectiveWeight {
            value: override_policy
                .and_then(|policy| policy.latency_weight)
                .unwrap_or(base.latency),
            overridden: override_policy.is_some_and(|policy| policy.latency_weight.is_some()),
        },
        availability: EffectiveWeight {
            value: base.availability,
            overridden: false,
        },
    }
}

fn weights_differ(left: &StrategyWeights, right: &StrategyWeights) -> bool {
    left.quality != right.quality
        || left.cost != right.cost
        || left.latency != right.latency
        || left.availability != right.availability
}

pub fn policy_view(config: &RoutingConfig, config_path: Option<&Path>) -> PolicyView {
    let defaults = RoutingConfig::default();
    let fallback_auto = StrategyWeights {
        quality: config.quality_weight,
        cost: config.cost_weight,
        latency: config.latency_weight,
        availability: 0.15,
    };
    let auto = config
        .strategy_weights
        .get("auto")
        .unwrap_or(&fallback_auto);

    let strategies = ["auto", "save", "fast"]
        .into_iter()
        .filter_map(|strategy| {
            let weights = config.strategy_weights.get(strategy)?;
            let default_weights = defaults.strategy_weights.get(strategy);
            Some(StrategyPolicyView {
                strategy: strategy.into(),
                weights: effective_weights(weights, None),
                overridden: config.configured_strategy_weights.contains(strategy)
                    || default_weights.is_none_or(|default| weights_differ(weights, default)),
            })
        })
        .collect();

    let tiers = [
        AgentTier::Simple,
        AgentTier::Medium,
        AgentTier::Reasoning,
        AgentTier::Complex,
    ]
    .into_iter()
    .map(|tier| {
        let override_policy = config.tier_policies.get(&tier);
        TierPolicyView {
            tier,
            weights: effective_weights(auto, override_policy),
            preferred_model_keywords: override_policy
                .map(|policy| policy.preferred_model_keywords.clone())
                .unwrap_or_default(),
            avoided_model_keywords: override_policy
                .map(|policy| policy.avoided_model_keywords.clone())
                .unwrap_or_default(),
            overridden: config.configured_tier_policies.contains(&tier)
                || override_policy.is_some(),
        }
    })
    .collect();

    PolicyView {
        default_strategy: config.default_strategy,
        fallback_enabled: config.fallback_enabled,
        circuit_breaker_threshold: config.circuit_breaker_threshold,
        circuit_breaker_timeout_secs: config.circuit_breaker_timeout_secs,
        strategies,
        tiers,
        editable: config_path.is_some(),
        config_path: config_path.map(|path| path.display().to_string()),
        editable_policy: config_path.map(|_| PolicyUpdate::from_config(config)),
    }
}

async fn get_policy(State(state): State<Arc<AppState>>) -> Json<PolicyView> {
    Json(policy_view(
        &state.routing_config,
        state.policy_config_path.as_deref(),
    ))
}

async fn put_policy(
    State(state): State<Arc<AppState>>,
    Json(update): Json<PolicyUpdate>,
) -> axum::response::Response {
    let Some(config_path) = state.policy_config_path.clone() else {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": {
                    "type": "policy_config_path_missing",
                    "message": "Policy editing requires an explicit Dispatcher config file"
                }
            })),
        )
            .into_response();
    };

    let config = match validate_policy_update(&update) {
        Ok(config) => config,
        Err(errors) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": {
                        "type": "policy_validation_error",
                        "message": "Routing policy validation failed",
                        "fields": errors
                    }
                })),
            )
                .into_response();
        }
    };

    let write_path = config_path.clone();
    let write_config = config.clone();
    match tokio::task::spawn_blocking(move || persist_policy_atomically(&write_path, &write_config))
        .await
    {
        Ok(Ok(())) => Json(PolicySaveResponse {
            saved: true,
            restart_required: true,
            config_path: config_path.display().to_string(),
            saved_policy: policy_view(&config, Some(&config_path)),
        })
        .into_response(),
        Ok(Err(error)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "policy_persistence_error",
                    "message": error.to_string()
                }
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "policy_persistence_task_error",
                    "message": error.to_string()
                }
            })),
        )
            .into_response(),
    }
}

fn validate_policy_update(update: &PolicyUpdate) -> Result<RoutingConfig, Vec<ValidationError>> {
    let mut errors = Vec::new();

    if !matches!(
        update.default_strategy,
        RoutingStrategy::Auto | RoutingStrategy::Save | RoutingStrategy::Fast
    ) {
        errors.push(validation_error(
            "default_strategy",
            "must be auto, save, or fast",
        ));
    }
    if !(1..=100).contains(&update.circuit_breaker_threshold) {
        errors.push(validation_error(
            "circuit_breaker_threshold",
            "must be between 1 and 100",
        ));
    }
    if !(1..=3_600).contains(&update.circuit_breaker_timeout_secs) {
        errors.push(validation_error(
            "circuit_breaker_timeout_secs",
            "must be between 1 and 3600 seconds",
        ));
    }

    let supported_strategies = ["auto", "save", "fast"];
    for strategy in update.strategy_weights.keys() {
        if !supported_strategies.contains(&strategy.as_str()) {
            errors.push(validation_error(
                format!("strategy_weights.{strategy}"),
                "unsupported strategy",
            ));
        }
    }
    for strategy in supported_strategies {
        match update.strategy_weights.get(strategy) {
            Some(weights) => validate_strategy_weights(strategy, weights, &mut errors),
            None => errors.push(validation_error(
                format!("strategy_weights.{strategy}"),
                "is required",
            )),
        }
    }

    let auto = update.strategy_weights.get("auto");
    for (tier, policy) in &update.tier_policies {
        validate_tier_policy(*tier, policy, auto, &mut errors);
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut config = RoutingConfig::default();
    config.default_strategy = update.default_strategy;
    config.fallback_enabled = update.fallback_enabled;
    config.circuit_breaker_threshold = update.circuit_breaker_threshold;
    config.circuit_breaker_timeout_secs = update.circuit_breaker_timeout_secs;
    config.strategy_weights = update.strategy_weights.clone();
    config.configured_strategy_weights = update.strategy_weights.keys().cloned().collect();
    config.tier_policies = update
        .tier_policies
        .iter()
        .map(|(tier, policy)| {
            (
                *tier,
                TierRoutingPolicy {
                    quality_weight: policy.quality_weight,
                    cost_weight: policy.cost_weight,
                    latency_weight: policy.latency_weight,
                    preferred_model_keywords: normalized_keywords(&policy.preferred_model_keywords),
                    avoided_model_keywords: normalized_keywords(&policy.avoided_model_keywords),
                },
            )
        })
        .collect();
    config.configured_tier_policies = config.tier_policies.keys().copied().collect();
    if let Some(auto) = config.strategy_weights.get("auto") {
        config.quality_weight = auto.quality;
        config.cost_weight = auto.cost;
        config.latency_weight = auto.latency;
    }

    Ok(config)
}

fn validate_strategy_weights(
    strategy: &str,
    weights: &StrategyWeights,
    errors: &mut Vec<ValidationError>,
) {
    for (name, value) in [
        ("quality", weights.quality),
        ("cost", weights.cost),
        ("latency", weights.latency),
        ("availability", weights.availability),
    ] {
        validate_weight(format!("strategy_weights.{strategy}.{name}"), value, errors);
    }
    validate_weight_sum(
        format!("strategy_weights.{strategy}"),
        weights.quality + weights.cost + weights.latency + weights.availability,
        errors,
    );
}

fn validate_tier_policy(
    tier: AgentTier,
    policy: &EditableTierPolicy,
    auto: Option<&StrategyWeights>,
    errors: &mut Vec<ValidationError>,
) {
    let tier_name = format!("{tier:?}").to_lowercase();
    for (name, value) in [
        ("quality_weight", policy.quality_weight),
        ("cost_weight", policy.cost_weight),
        ("latency_weight", policy.latency_weight),
    ] {
        if let Some(value) = value {
            validate_weight(format!("tier_policies.{tier_name}.{name}"), value, errors);
        }
    }

    if let Some(auto) = auto {
        validate_weight_sum(
            format!("tier_policies.{tier_name}"),
            policy.quality_weight.unwrap_or(auto.quality)
                + policy.cost_weight.unwrap_or(auto.cost)
                + policy.latency_weight.unwrap_or(auto.latency)
                + auto.availability,
            errors,
        );
    }

    validate_keywords(
        format!("tier_policies.{tier_name}.preferred_model_keywords"),
        &policy.preferred_model_keywords,
        errors,
    );
    validate_keywords(
        format!("tier_policies.{tier_name}.avoided_model_keywords"),
        &policy.avoided_model_keywords,
        errors,
    );
}

fn validate_weight(field: String, value: f64, errors: &mut Vec<ValidationError>) {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        errors.push(validation_error(field, "must be between 0 and 1"));
    }
}

fn validate_weight_sum(field: String, sum: f64, errors: &mut Vec<ValidationError>) {
    if (sum - 1.0).abs() > 0.000_001 {
        errors.push(validation_error(
            field,
            "effective weights must add up to 1",
        ));
    }
}

fn validate_keywords(field: String, keywords: &[String], errors: &mut Vec<ValidationError>) {
    if keywords.len() > 20 {
        errors.push(validation_error(
            field.clone(),
            "must contain at most 20 keywords",
        ));
    }

    let mut seen = HashSet::new();
    for (index, keyword) in keywords.iter().enumerate() {
        let trimmed = keyword.trim();
        if trimmed.is_empty() {
            errors.push(validation_error(
                format!("{field}.{index}"),
                "must not be empty",
            ));
        } else if trimmed.chars().count() > 64 {
            errors.push(validation_error(
                format!("{field}.{index}"),
                "must be at most 64 characters",
            ));
        } else if trimmed.chars().any(char::is_control) {
            errors.push(validation_error(
                format!("{field}.{index}"),
                "must not contain control characters",
            ));
        }

        if !seen.insert(trimmed.to_lowercase()) {
            errors.push(validation_error(
                format!("{field}.{index}"),
                "must not contain duplicate keywords",
            ));
        }
    }
}

fn normalized_keywords(keywords: &[String]) -> Vec<String> {
    keywords
        .iter()
        .map(|keyword| keyword.trim().to_string())
        .collect()
}

fn validation_error(field: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError {
        field: field.into(),
        message: message.into(),
    }
}

fn persist_policy_atomically(path: &Path, config: &RoutingConfig) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dispatcher.toml");
    let temp_path: PathBuf = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let serialized = toml::to_string_pretty(config)?;

    let result = (|| -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp_path, path)?;
        if let Ok(directory) = std::fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/policy", get(get_policy).put(put_policy))
}

#[cfg(test)]
mod tests {
    use dispatcher_engine::{AgentTier, RoutingConfig, StrategyWeights, TierRoutingPolicy};

    use super::*;

    #[test]
    fn policy_view_reports_effective_tier_weights_and_override_sources() {
        let mut config = RoutingConfig::default();
        config.strategy_weights.insert(
            "auto".into(),
            StrategyWeights {
                quality: 0.45,
                cost: 0.25,
                latency: 0.15,
                availability: 0.15,
            },
        );
        config.tier_policies.insert(
            AgentTier::Reasoning,
            TierRoutingPolicy {
                quality_weight: Some(0.72),
                cost_weight: None,
                latency_weight: Some(0.08),
                preferred_model_keywords: vec!["sonnet".into(), "gpt".into()],
                avoided_model_keywords: vec!["mini".into()],
            },
        );

        let view = policy_view(&config, None);
        let reasoning = view
            .tiers
            .iter()
            .find(|tier| tier.tier == AgentTier::Reasoning)
            .unwrap();

        assert_eq!(reasoning.weights.quality.value, 0.72);
        assert!(reasoning.weights.quality.overridden);
        assert_eq!(reasoning.weights.cost.value, 0.25);
        assert!(!reasoning.weights.cost.overridden);
        assert_eq!(reasoning.weights.latency.value, 0.08);
        assert!(reasoning.weights.latency.overridden);
        assert_eq!(reasoning.weights.availability.value, 0.15);
        assert!(!reasoning.weights.availability.overridden);
        assert_eq!(reasoning.preferred_model_keywords, vec!["sonnet", "gpt"]);
        assert_eq!(reasoning.avoided_model_keywords, vec!["mini"]);
    }

    #[test]
    fn policy_view_marks_explicit_strategy_even_when_values_match_defaults() {
        let config = RoutingConfig::from_toml_str(
            r#"
[strategy_weights.fast]
quality = 0.25
cost = 0.10
latency = 0.55
availability = 0.10
"#,
        )
        .unwrap();

        let view = policy_view(&config, None);
        let fast = view
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "fast")
            .unwrap();

        assert!(fast.overridden);
    }

    #[test]
    fn policy_update_rejects_invalid_weights_and_unsupported_strategy() {
        let mut update = PolicyUpdate::from_config(&RoutingConfig::default());
        update.default_strategy = RoutingStrategy::Manual;
        update.strategy_weights.get_mut("auto").unwrap().quality = 1.2;

        let errors = validate_policy_update(&update).unwrap_err();

        assert!(errors.iter().any(|error| error.field == "default_strategy"));
        assert!(errors
            .iter()
            .any(|error| error.field == "strategy_weights.auto.quality"));
    }

    #[test]
    fn policy_update_rejects_unknown_secret_fields() {
        let result = serde_json::from_value::<PolicyUpdate>(serde_json::json!({
            "default_strategy": "auto",
            "fallback_enabled": true,
            "circuit_breaker_threshold": 3,
            "circuit_breaker_timeout_secs": 30,
            "strategy_weights": {
                "auto": {"quality":0.5,"cost":0.2,"latency":0.15,"availability":0.15},
                "save": {"quality":0.2,"cost":0.6,"latency":0.1,"availability":0.1},
                "fast": {"quality":0.25,"cost":0.1,"latency":0.55,"availability":0.1}
            },
            "tier_policies": {},
            "openai_api_key": "must-not-be-accepted"
        }));

        assert!(result.is_err());
    }

    #[test]
    fn policy_update_validates_keywords_and_circuit_breaker_values() {
        let mut update = PolicyUpdate::from_config(&RoutingConfig::default());
        update.circuit_breaker_threshold = 0;
        update.circuit_breaker_timeout_secs = 3_601;
        update.tier_policies.insert(
            AgentTier::Simple,
            EditableTierPolicy {
                quality_weight: Some(0.5),
                cost_weight: Some(0.2),
                latency_weight: Some(0.15),
                preferred_model_keywords: vec!["".into(), "x".repeat(65)],
                avoided_model_keywords: vec![],
            },
        );

        let errors = validate_policy_update(&update).unwrap_err();

        assert!(errors
            .iter()
            .any(|error| error.field == "circuit_breaker_threshold"));
        assert!(errors
            .iter()
            .any(|error| error.field == "circuit_breaker_timeout_secs"));
        assert!(errors
            .iter()
            .any(|error| error.field.starts_with("tier_policies.simple")));
    }

    #[test]
    fn policy_update_is_persisted_as_parseable_routing_config() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-policy-test-{}/dispatcher.toml",
            uuid::Uuid::new_v4()
        ));
        let mut update = PolicyUpdate::from_config(&RoutingConfig::default());
        update.fallback_enabled = false;
        update.tier_policies.insert(
            AgentTier::Reasoning,
            EditableTierPolicy {
                quality_weight: Some(0.7),
                cost_weight: Some(0.05),
                latency_weight: Some(0.1),
                preferred_model_keywords: vec!["gpt".into()],
                avoided_model_keywords: vec!["mini".into()],
            },
        );

        let config = validate_policy_update(&update).unwrap();
        persist_policy_atomically(&path, &config).unwrap();

        let saved = RoutingConfig::from_toml_file(&path).unwrap();
        assert!(!saved.fallback_enabled);
        assert_eq!(
            saved
                .tier_policies
                .get(&AgentTier::Reasoning)
                .unwrap()
                .preferred_model_keywords,
            vec!["gpt"]
        );

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
