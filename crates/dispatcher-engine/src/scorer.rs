use crate::types::*;
use std::collections::HashMap;

/// Provider 评分器 — 根据请求特征和路由策略为每个候选 provider 打分
pub struct ProviderScorer {
    config: RoutingConfig,
}

impl ProviderScorer {
    const PREFERRED_KEYWORD_BONUS: f64 = 0.04;
    const AVOIDED_KEYWORD_PENALTY: f64 = -0.08;

    pub fn new(config: RoutingConfig) -> Self {
        Self { config }
    }

    /// 为所有候选 provider 打分
    pub fn score_all(
        &self,
        capabilities: &[ProviderCapability],
        features: &RequestFeatures,
        strategy: RoutingStrategy,
    ) -> Vec<ProviderScore> {
        self.score_all_with_health(capabilities, features, strategy, &HashMap::new())
    }

    pub fn score_all_with_health(
        &self,
        capabilities: &[ProviderCapability],
        features: &RequestFeatures,
        strategy: RoutingStrategy,
        health: &HashMap<String, ProviderHealthSnapshot>,
    ) -> Vec<ProviderScore> {
        let mut weights = self
            .config
            .strategy_weights
            .get(match strategy {
                RoutingStrategy::Auto => "auto",
                RoutingStrategy::Save => "save",
                RoutingStrategy::Fast => "fast",
                RoutingStrategy::Manual | RoutingStrategy::Random => "auto",
            })
            .cloned()
            .unwrap_or(StrategyWeights {
                quality: 0.5,
                cost: 0.2,
                latency: 0.15,
                availability: 0.15,
            });
        let tier_policy = if matches!(strategy, RoutingStrategy::Save) {
            None
        } else {
            self.config.tier_policies.get(&features.agent_tier)
        };
        if let Some(policy) = tier_policy {
            if let Some(weight) = policy.quality_weight {
                weights.quality = weight;
            }
            if let Some(weight) = policy.cost_weight {
                weights.cost = weight;
            }
            if let Some(weight) = policy.latency_weight {
                weights.latency = weight;
            }
        }

        capabilities
            .iter()
            .flat_map(|cap| {
                cap.supported_models.iter().map(move |model| {
                    let policy_effect =
                        Self::policy_effect(features.agent_tier, tier_policy, cap, model);
                    let runtime_health = health.get(&cap.provider_id);
                    let (availability_score, availability) =
                        Self::runtime_availability(runtime_health);
                    let avg_latency_ms = runtime_health
                        .filter(|snapshot| {
                            snapshot.sample_count >= 3 && snapshot.avg_latency_ms > 0
                        })
                        .map(|snapshot| snapshot.avg_latency_ms)
                        .unwrap_or(model.avg_latency_ms);
                    let mut score = ProviderScore {
                        provider_id: cap.provider_id.clone(),
                        model_id: model.model_id.clone(),
                        total_score: 0.0,
                        quality_score: self.score_quality(model, features),
                        cost_score: self.score_cost(model),
                        latency_score: Self::score_latency_ms(avg_latency_ms),
                        availability_score,
                        estimated_cost_per_1k: model.input_cost_per_1k + model.output_cost_per_1k,
                        input_cost_per_1k: model.input_cost_per_1k,
                        output_cost_per_1k: model.output_cost_per_1k,
                        avg_latency_ms,
                        availability,
                        policy_reason: policy_effect.reason,
                        handoff_certification: model.handoff_certification.clone(),
                        score_breakdown: ScoreBreakdown {
                            weighted_quality: 0.0,
                            weighted_cost: 0.0,
                            weighted_latency: 0.0,
                            weighted_availability: 0.0,
                            weighted_policy: 0.0,
                        },
                    };

                    score.score_breakdown = ScoreBreakdown {
                        weighted_quality: score.quality_score * weights.quality,
                        weighted_cost: score.cost_score * weights.cost,
                        weighted_latency: score.latency_score * weights.latency,
                        weighted_availability: score.availability_score * weights.availability,
                        weighted_policy: policy_effect.adjustment,
                    };
                    let total = score.score_breakdown.weighted_quality
                        + score.score_breakdown.weighted_cost
                        + score.score_breakdown.weighted_latency
                        + score.score_breakdown.weighted_availability
                        + score.score_breakdown.weighted_policy;
                    score.total_score = total.clamp(0.0, 1.0);

                    score
                })
            })
            .collect()
    }

    fn runtime_availability(health: Option<&ProviderHealthSnapshot>) -> (f64, AvailabilityStatus) {
        let Some(health) = health.filter(|snapshot| snapshot.sample_count >= 3) else {
            return (1.0, AvailabilityStatus::Available);
        };

        let score = health.success_rate.clamp(0.1, 1.0);
        let status = if health.success_rate >= 0.9 {
            AvailabilityStatus::Available
        } else {
            AvailabilityStatus::Degraded
        };
        (score, status)
    }

    fn policy_effect(
        tier: AgentTier,
        policy: Option<&TierRoutingPolicy>,
        capability: &ProviderCapability,
        model: &ModelInfo,
    ) -> PolicyEffect {
        let Some(policy) = policy else {
            return PolicyEffect::default();
        };

        let mut reasons = Vec::new();
        let mut adjustment = 0.0;

        if policy.quality_weight.is_some()
            || policy.cost_weight.is_some()
            || policy.latency_weight.is_some()
        {
            reasons.push(format!(
                "{} policy: tier weights override",
                format!("{:?}", tier).to_lowercase()
            ));
        }

        let preferred =
            Self::matching_keywords(&policy.preferred_model_keywords, capability, model);
        if !preferred.is_empty() {
            adjustment += Self::PREFERRED_KEYWORD_BONUS;
            reasons.push(format!("preferred keyword match: {}", preferred.join(", ")));
        }

        let avoided = Self::matching_keywords(&policy.avoided_model_keywords, capability, model);
        if !avoided.is_empty() {
            adjustment += Self::AVOIDED_KEYWORD_PENALTY;
            reasons.push(format!("avoided keyword match: {}", avoided.join(", ")));
        }

        PolicyEffect {
            adjustment,
            reason: if reasons.is_empty() {
                None
            } else {
                Some(reasons.join("; "))
            },
        }
    }

    fn matching_keywords(
        keywords: &[String],
        capability: &ProviderCapability,
        model: &ModelInfo,
    ) -> Vec<String> {
        if keywords.is_empty() {
            return Vec::new();
        }

        let haystack = format!(
            "{} {} {} {}",
            capability.provider_id, capability.provider_name, model.model_id, model.display_name
        )
        .to_lowercase();

        keywords
            .iter()
            .filter_map(|keyword| {
                let normalized = keyword.trim().to_lowercase();
                if normalized.is_empty() {
                    return None;
                }
                haystack.contains(&normalized).then_some(normalized)
            })
            .collect()
    }

    fn score_quality(&self, model: &ModelInfo, features: &RequestFeatures) -> f64 {
        let base = model.quality_score;
        let cost = model.input_cost_per_1k + model.output_cost_per_1k;
        let low_cost = cost <= 0.0006;
        let high_cost = cost >= 0.006;
        let low_latency = model.avg_latency_ms > 0 && model.avg_latency_ms <= 800;
        let high_latency = model.avg_latency_ms >= 3_000;
        let high_quality = model.quality_score >= 0.88;
        let good_quality = model.quality_score >= 0.80;
        let long_context = model.max_tokens >= 100_000;

        let task_bonus: f64 = match features.task_type {
            TaskType::Code => {
                if high_quality && long_context {
                    0.08
                } else if good_quality {
                    0.04
                } else {
                    -0.04
                }
            }
            TaskType::Analysis => {
                if high_quality {
                    0.08
                } else if good_quality {
                    0.05
                } else {
                    -0.03
                }
            }
            TaskType::Chat | TaskType::Translation => {
                if low_cost && low_latency {
                    0.06
                } else if high_cost || high_latency {
                    -0.04
                } else {
                    0.0
                }
            }
            TaskType::Summarization if good_quality => 0.03,
            TaskType::Summarization => 0.0,
            _ => 0.0,
        };

        let ctx_bonus = if features.is_long_context {
            if model.max_tokens > 100_000 {
                0.05
            } else {
                -0.1
            }
        } else {
            0.0
        };

        let tool_bonus = if features.has_tools {
            if good_quality && long_context {
                0.03
            } else {
                -0.02
            }
        } else {
            0.0
        };

        let complexity_bonus = if features.complexity_score > 0.5 {
            if high_quality && long_context {
                0.06
            } else if high_quality {
                0.04
            } else {
                -0.04
            }
        } else {
            0.0
        };

        let tier_bonus = match features.agent_tier {
            AgentTier::Simple => {
                if low_cost && low_latency {
                    0.08
                } else if high_cost || high_latency {
                    -0.06
                } else {
                    0.0
                }
            }
            AgentTier::Medium => {
                if good_quality && (low_cost || low_latency) {
                    0.03
                } else {
                    0.0
                }
            }
            AgentTier::Reasoning => {
                if high_quality && long_context {
                    0.10
                } else if high_quality {
                    0.06
                } else {
                    -0.06
                }
            }
            AgentTier::Complex => {
                if high_quality && long_context {
                    0.14
                } else if high_quality {
                    0.08
                } else {
                    -0.10
                }
            }
        };

        (base + task_bonus + ctx_bonus + tool_bonus + complexity_bonus + tier_bonus).clamp(0.0, 1.0)
    }

    fn score_cost(&self, model: &ModelInfo) -> f64 {
        let cost = model.input_cost_per_1k + model.output_cost_per_1k;
        if cost <= 0.0 {
            return 1.0;
        }
        let score = 1.0 - (cost * 1000.0).ln() / 10.0;
        score.clamp(0.0, 1.0)
    }

    fn score_latency_ms(avg_latency_ms: u64) -> f64 {
        if avg_latency_ms == 0 {
            return 0.8;
        }
        let score = 1.0 - (avg_latency_ms as f64 / 10_000.0);
        score.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_capability(
        provider_id: &str,
        model_id: &str,
        quality: f64,
        cost: f64,
        latency: u64,
    ) -> ProviderCapability {
        ProviderCapability {
            provider_id: provider_id.into(),
            provider_name: provider_id.into(),
            supported_models: vec![ModelInfo {
                model_id: model_id.into(),
                display_name: model_id.into(),
                input_cost_per_1k: cost,
                output_cost_per_1k: cost * 2.0,
                pricing_source: None,
                pricing_updated_at: None,
                supports_streaming: None,
                supports_tools: None,
                supports_vision: None,
                max_tokens: 4096,
                quality_score: quality,
                avg_latency_ms: latency,
                handoff_certification: HandoffCertification::default(),
            }],
            base_url: String::new(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            max_context_length: 128_000,
        }
    }

    #[test]
    fn auto_strategy_prefers_quality() {
        let caps = vec![
            make_capability("openai", "gpt-4", 0.9, 0.03, 2000),
            make_capability("gemini", "gemini-flash", 0.6, 0.00015, 500),
        ];

        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 1000,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.6,
            task_type: TaskType::Code, // 代码任务偏好 coding 模型
            agent_tier: AgentTier::Reasoning,
            is_long_context: false,
        };

        let mut scores = scorer.score_all(&caps, &features, RoutingStrategy::Auto);
        scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

        // 代码任务下 GPT-4（coding模型）排前面
        assert_eq!(scores[0].model_id, "gpt-4");
    }

    #[test]
    fn save_strategy_prefers_cost() {
        let caps = vec![
            make_capability("openai", "gpt-4", 0.9, 0.03, 2000),
            make_capability("gemini", "gemini-flash", 0.6, 0.00015, 500),
        ];

        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 1000,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.3,
            task_type: TaskType::Chat,
            agent_tier: AgentTier::Simple,
            is_long_context: false,
        };

        let mut scores = scorer.score_all(&caps, &features, RoutingStrategy::Save);
        scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

        // Save 策略下 Gemini Flash 应该排前面（成本优先）
        assert_eq!(scores[0].model_id, "gemini-flash");
    }

    #[test]
    fn score_includes_weighted_breakdown() {
        let caps = vec![make_capability("openai", "gpt-4", 0.9, 0.03, 2000)];
        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 1000,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.6,
            task_type: TaskType::Code,
            agent_tier: AgentTier::Reasoning,
            is_long_context: false,
        };

        let scores = scorer.score_all(&caps, &features, RoutingStrategy::Auto);
        let score = &scores[0];

        assert!(score.score_breakdown.weighted_quality > 0.0);
        assert!(score.score_breakdown.weighted_cost > 0.0);
        assert!(score.score_breakdown.weighted_latency > 0.0);
        assert!(score.score_breakdown.weighted_availability > 0.0);
        assert!(
            (score.total_score
                - (score.score_breakdown.weighted_quality
                    + score.score_breakdown.weighted_cost
                    + score.score_breakdown.weighted_latency
                    + score.score_breakdown.weighted_availability
                    + score.score_breakdown.weighted_policy)
                    .clamp(0.0, 1.0))
            .abs()
                < 0.000001
        );
    }

    #[test]
    fn estimated_cost_per_1k_includes_input_and_output_costs() {
        let caps = vec![make_capability("budget", "qwen-fast", 0.7, 0.001, 500)];
        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 1000,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.1,
            task_type: TaskType::Chat,
            agent_tier: AgentTier::Simple,
            is_long_context: false,
        };

        let scores = scorer.score_all(&caps, &features, RoutingStrategy::Save);

        assert!((scores[0].estimated_cost_per_1k - 0.003).abs() < 0.000001);
    }

    #[test]
    fn save_strategy_keeps_cost_weight_even_when_tier_policy_exists() {
        let caps = vec![make_capability("budget", "qwen-fast", 0.7, 0.0001, 500)];
        let mut config = RoutingConfig::default();
        config.tier_policies.insert(
            AgentTier::Reasoning,
            TierRoutingPolicy {
                quality_weight: Some(0.65),
                cost_weight: Some(0.10),
                latency_weight: Some(0.10),
                preferred_model_keywords: Vec::new(),
                avoided_model_keywords: Vec::new(),
            },
        );

        let scorer = ProviderScorer::new(config);
        let features = RequestFeatures {
            estimated_tokens: 1000,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.6,
            task_type: TaskType::Analysis,
            agent_tier: AgentTier::Reasoning,
            is_long_context: false,
        };

        let scores = scorer.score_all(&caps, &features, RoutingStrategy::Save);

        assert!(
            scores[0].score_breakdown.weighted_cost > scores[0].score_breakdown.weighted_quality
        );
        assert!(scores[0].policy_reason.is_none());
    }

    #[test]
    fn equivalent_models_do_not_get_boosted_by_name_keywords() {
        let caps = vec![
            make_capability("a", "vendor-flash-model", 0.75, 0.0002, 500),
            make_capability("b", "vendor-steady-model", 0.75, 0.0002, 500),
        ];

        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 128,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.05,
            task_type: TaskType::Chat,
            agent_tier: AgentTier::Simple,
            is_long_context: false,
        };

        let scores = scorer.score_all(&caps, &features, RoutingStrategy::Auto);
        assert!((scores[0].total_score - scores[1].total_score).abs() < f64::EPSILON);
    }

    #[test]
    fn configured_tier_policy_keywords_adjust_scores() {
        let caps = vec![
            make_capability("a", "vendor-flash-model", 0.75, 0.0002, 500),
            make_capability("b", "vendor-steady-model", 0.75, 0.0002, 500),
        ];
        let mut config = RoutingConfig::default();
        config.tier_policies.insert(
            AgentTier::Simple,
            TierRoutingPolicy {
                quality_weight: Some(0.10),
                cost_weight: Some(0.65),
                latency_weight: Some(0.15),
                preferred_model_keywords: vec!["flash".into()],
                avoided_model_keywords: vec!["steady".into()],
            },
        );

        let scorer = ProviderScorer::new(config);
        let features = RequestFeatures {
            estimated_tokens: 128,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.05,
            task_type: TaskType::Chat,
            agent_tier: AgentTier::Simple,
            is_long_context: false,
        };

        let scores = scorer.score_all(&caps, &features, RoutingStrategy::Auto);
        let preferred = scores
            .iter()
            .find(|score| score.model_id == "vendor-flash-model")
            .unwrap();
        let avoided = scores
            .iter()
            .find(|score| score.model_id == "vendor-steady-model")
            .unwrap();

        assert!(preferred.total_score > avoided.total_score);
        assert!(preferred.score_breakdown.weighted_policy > 0.0);
        assert!(avoided.score_breakdown.weighted_policy < 0.0);
        assert_eq!(
            preferred.policy_reason.as_deref(),
            Some("simple policy: tier weights override; preferred keyword match: flash")
        );
        assert_eq!(
            avoided.policy_reason.as_deref(),
            Some("simple policy: tier weights override; avoided keyword match: steady")
        );
    }

    #[test]
    fn runtime_health_penalizes_unreliable_provider() {
        let caps = vec![
            make_capability("healthy", "same-model-a", 0.82, 0.001, 700),
            make_capability("unreliable", "same-model-b", 0.82, 0.001, 700),
        ];
        let scorer = ProviderScorer::new(RoutingConfig::default());
        let features = RequestFeatures {
            estimated_tokens: 512,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.3,
            task_type: TaskType::Code,
            agent_tier: AgentTier::Medium,
            is_long_context: false,
        };
        let health = HashMap::from([
            (
                "healthy".to_string(),
                ProviderHealthSnapshot {
                    provider_id: "healthy".into(),
                    sample_count: 20,
                    success_rate: 0.98,
                    avg_latency_ms: 650,
                },
            ),
            (
                "unreliable".to_string(),
                ProviderHealthSnapshot {
                    provider_id: "unreliable".into(),
                    sample_count: 20,
                    success_rate: 0.40,
                    avg_latency_ms: 650,
                },
            ),
        ]);

        let scores = scorer.score_all_with_health(&caps, &features, RoutingStrategy::Auto, &health);
        let healthy = scores
            .iter()
            .find(|score| score.provider_id == "healthy")
            .unwrap();
        let unreliable = scores
            .iter()
            .find(|score| score.provider_id == "unreliable")
            .unwrap();

        assert!(healthy.total_score > unreliable.total_score);
        assert_eq!(healthy.availability, AvailabilityStatus::Available);
        assert_eq!(unreliable.availability, AvailabilityStatus::Degraded);
        assert!(healthy.availability_score > unreliable.availability_score);
    }

    #[test]
    fn tier_policy_can_override_default_scoring_with_weights() {
        let caps = vec![
            make_capability("premium", "claude-opus-4-7", 0.98, 0.015, 2200),
            make_capability("budget", "qwen-flash", 0.65, 0.0001, 450),
        ];
        let mut config = RoutingConfig::default();
        config.tier_policies.insert(
            AgentTier::Simple,
            TierRoutingPolicy {
                quality_weight: Some(0.10),
                cost_weight: Some(0.65),
                latency_weight: Some(0.15),
                preferred_model_keywords: vec!["flash".into()],
                avoided_model_keywords: vec!["opus".into()],
            },
        );

        let scorer = ProviderScorer::new(config);
        let features = RequestFeatures {
            estimated_tokens: 128,
            has_tools: false,
            has_images: false,
            is_streaming: false,
            complexity_score: 0.05,
            task_type: TaskType::Chat,
            agent_tier: AgentTier::Simple,
            is_long_context: false,
        };

        let mut scores = scorer.score_all(&caps, &features, RoutingStrategy::Auto);
        scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

        assert_eq!(scores[0].model_id, "qwen-flash");
        assert_eq!(
            scores[0].policy_reason.as_deref(),
            Some("simple policy: tier weights override; preferred keyword match: flash")
        );
    }
}

#[derive(Debug, Default)]
struct PolicyEffect {
    adjustment: f64,
    reason: Option<String>,
}
