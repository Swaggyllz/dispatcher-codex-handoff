use crate::types::*;
use std::cmp::Ordering;

/// 路由选择器 — 根据评分结果选择最佳 provider
pub struct RouteSelector {
    config: RoutingConfig,
}

impl RouteSelector {
    pub fn new(config: RoutingConfig) -> Self {
        Self { config }
    }

    /// 选择最佳路由
    pub fn select(
        &self,
        mut scores: Vec<ProviderScore>,
        strategy: RoutingStrategy,
        agent_tier: AgentTier,
        excluded_providers: &[String],
    ) -> Option<RoutingDecision> {
        let start = std::time::Instant::now();

        // 过滤掉被排除的 provider
        scores.retain(|s| !excluded_providers.contains(&s.provider_id));

        // 过滤掉不可用的 provider
        scores.retain(|s| s.availability != AvailabilityStatus::Unavailable);

        if scores.is_empty() {
            return None;
        }

        match strategy {
            RoutingStrategy::Auto => {
                // 直接选评分最高的 — 任务感知评分已确保模型匹配任务
                scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());
            }
            RoutingStrategy::Save => {
                // 省钱模式的语义更硬：先守住质量底线，再在合格候选中选预估成本最低的。
                Self::sort_for_save_strategy(&mut scores, agent_tier);
            }
            RoutingStrategy::Fast => {
                Self::sort_for_fast_strategy(&mut scores, agent_tier);
            }
            RoutingStrategy::Random => {
                // 按总分加权随机
                let total: f64 = scores.iter().map(|s| s.total_score).sum();
                if total > 0.0 {
                    let mut rng = rand::f64();
                    for score in &scores {
                        rng -= score.total_score / total;
                        if rng <= 0.0 {
                            let decision_time_ms = start.elapsed().as_millis() as u64;
                            let policy_reason = score.policy_reason.clone();
                            let decision_reason =
                                Self::decision_reason(score, agent_tier, "weighted random draw");
                            return Some(RoutingDecision {
                                provider_id: score.provider_id.clone(),
                                model_id: score.model_id.clone(),
                                strategy,
                                agent_tier,
                                candidates: scores,
                                decision_time_ms,
                                is_fallback: false,
                                fallback_reason: None,
                                fallback_chain: Vec::new(),
                                policy_reason,
                                decision_reason,
                                excluded_candidates: Vec::new(),
                            });
                        }
                    }
                }
                // 如果加权随机失败，选第一个
                scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());
            }
            RoutingStrategy::Manual => {
                // 手动模式不做选择，由调用者指定
            }
        }

        let best = scores.first()?;
        let policy_reason = best.policy_reason.clone();
        let decision_basis = match strategy {
            RoutingStrategy::Save => {
                "lowest estimated cost within tier-aware quality guard, with near-equal costs broken by total score"
            }
            RoutingStrategy::Fast => {
                "lowest latency within tier-aware quality guard, with near-equal latency broken by total score"
            }
            _ => "highest total score",
        };
        let decision_reason = Self::decision_reason(best, agent_tier, decision_basis);
        let decision_time_ms = start.elapsed().as_millis() as u64;

        Some(RoutingDecision {
            provider_id: best.provider_id.clone(),
            model_id: best.model_id.clone(),
            strategy,
            agent_tier,
            candidates: scores,
            decision_time_ms,
            is_fallback: false,
            fallback_reason: None,
            fallback_chain: Vec::new(),
            policy_reason,
            decision_reason,
            excluded_candidates: Vec::new(),
        })
    }

    fn sort_for_save_strategy(scores: &mut [ProviderScore], agent_tier: AgentTier) {
        let quality_floor = Self::quality_floor(scores, agent_tier);

        scores.sort_by(|a, b| {
            let a_viable = a.quality_score >= quality_floor;
            let b_viable = b.quality_score >= quality_floor;

            match (a_viable, b_viable) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (true, true) => Self::compare_cost_then_score(a, b),
                (false, false) => b.total_score.partial_cmp(&a.total_score).unwrap(),
            }
        });
    }

    fn sort_for_fast_strategy(scores: &mut [ProviderScore], agent_tier: AgentTier) {
        let quality_floor = Self::quality_floor(scores, agent_tier);

        scores.sort_by(|a, b| {
            let a_viable = a.quality_score >= quality_floor;
            let b_viable = b.quality_score >= quality_floor;

            match (a_viable, b_viable) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (true, true) => Self::compare_latency_then_score(a, b),
                (false, false) => b.total_score.partial_cmp(&a.total_score).unwrap(),
            }
        });
    }

    fn quality_floor(scores: &[ProviderScore], agent_tier: AgentTier) -> f64 {
        let max_quality = scores
            .iter()
            .map(|score| score.quality_score)
            .fold(0.0, f64::max);
        let (absolute_floor, max_drop): (f64, f64) = match agent_tier {
            AgentTier::Simple => (0.55, 0.35),
            AgentTier::Medium => (0.65, 0.25),
            AgentTier::Reasoning => (0.82, 0.15),
            AgentTier::Complex => (0.88, 0.10),
        };

        max_quality.min(absolute_floor.max(max_quality - max_drop))
    }

    fn compare_cost_then_score(a: &ProviderScore, b: &ProviderScore) -> Ordering {
        const NEAR_EQUAL_COST_RATIO: f64 = 0.10;

        let lower_cost = a.estimated_cost_per_1k.min(b.estimated_cost_per_1k);
        let cost_delta = (a.estimated_cost_per_1k - b.estimated_cost_per_1k).abs();
        if lower_cost > 0.0 && cost_delta / lower_cost <= NEAR_EQUAL_COST_RATIO {
            return b
                .total_score
                .partial_cmp(&a.total_score)
                .unwrap()
                .then_with(|| a.avg_latency_ms.cmp(&b.avg_latency_ms));
        }

        a.estimated_cost_per_1k
            .partial_cmp(&b.estimated_cost_per_1k)
            .unwrap()
            .then_with(|| b.total_score.partial_cmp(&a.total_score).unwrap())
            .then_with(|| a.avg_latency_ms.cmp(&b.avg_latency_ms))
    }

    fn compare_latency_then_score(a: &ProviderScore, b: &ProviderScore) -> Ordering {
        const NEAR_EQUAL_LATENCY_RATIO: f64 = 0.10;

        let lower_latency = a.avg_latency_ms.min(b.avg_latency_ms);
        let latency_delta = a.avg_latency_ms.abs_diff(b.avg_latency_ms);
        if lower_latency > 0
            && latency_delta as f64 / lower_latency as f64 <= NEAR_EQUAL_LATENCY_RATIO
        {
            return b
                .total_score
                .partial_cmp(&a.total_score)
                .unwrap()
                .then_with(|| {
                    a.estimated_cost_per_1k
                        .partial_cmp(&b.estimated_cost_per_1k)
                        .unwrap()
                });
        }

        a.avg_latency_ms
            .cmp(&b.avg_latency_ms)
            .then_with(|| b.total_score.partial_cmp(&a.total_score).unwrap())
            .then_with(|| {
                a.estimated_cost_per_1k
                    .partial_cmp(&b.estimated_cost_per_1k)
                    .unwrap()
            })
    }

    fn decision_reason(score: &ProviderScore, agent_tier: AgentTier, basis: &str) -> String {
        format!(
            "Selected {}/{} for {:?} tier by {}: total={:.3}, quality={:.3}, cost={:.3}, latency={:.3}, availability={:.3}",
            score.provider_id,
            score.model_id,
            agent_tier,
            basis,
            score.total_score,
            score.score_breakdown.weighted_quality,
            score.score_breakdown.weighted_cost,
            score.score_breakdown.weighted_latency,
            score.score_breakdown.weighted_availability,
        )
        .to_lowercase()
    }

    /// 获取 fallback 候选（同 model 不同 provider，或降级 model）
    pub fn get_fallback(
        &self,
        original: &RoutingDecision,
        mut scores: Vec<ProviderScore>,
    ) -> Option<RoutingDecision> {
        if !self.config.fallback_enabled {
            return None;
        }

        // 第一层 fallback：同 model 不同 provider
        // 先提取备选信息，避免 borrow 冲突
        let fallback_info = scores
            .iter()
            .find(|s| s.model_id == original.model_id && s.provider_id != original.provider_id)
            .map(|s| (s.provider_id.clone(), s.model_id.clone()));

        if let Some((provider_id, model_id)) = fallback_info {
            return Some(RoutingDecision {
                provider_id: provider_id.clone(),
                model_id: model_id.clone(),
                strategy: original.strategy,
                agent_tier: original.agent_tier,
                candidates: scores,
                decision_time_ms: 0,
                is_fallback: true,
                fallback_reason: Some(format!(
                    "Primary provider {} unavailable, falling back to {}",
                    original.provider_id, provider_id
                )),
                fallback_chain: original.fallback_chain.clone(),
                policy_reason: original.policy_reason.clone(),
                decision_reason: format!(
                    "Fallback selected {}/{} because primary provider {} was unavailable",
                    provider_id, model_id, original.provider_id
                ),
                excluded_candidates: original.excluded_candidates.clone(),
            });
        }

        // 第二层 fallback：降级到成本更低的 model
        scores.sort_by(|a, b| {
            a.estimated_cost_per_1k
                .partial_cmp(&b.estimated_cost_per_1k)
                .unwrap()
        });

        if let Some(fallback) = scores.first() {
            let policy_reason = fallback.policy_reason.clone();
            let provider_id = fallback.provider_id.clone();
            let model_id = fallback.model_id.clone();
            let decision_reason = format!(
                "Fallback selected {}/{} as the lowest-cost compatible candidate",
                provider_id, model_id
            );
            return Some(RoutingDecision {
                provider_id,
                model_id,
                strategy: original.strategy,
                agent_tier: original.agent_tier,
                candidates: scores,
                decision_time_ms: 0,
                is_fallback: true,
                fallback_reason: Some(
                    "All primary providers unavailable, using cheapest fallback".into(),
                ),
                fallback_chain: original.fallback_chain.clone(),
                policy_reason,
                decision_reason,
                excluded_candidates: original.excluded_candidates.clone(),
            });
        }

        None
    }

    pub fn get_fallback_candidates(
        &self,
        original: &RoutingDecision,
        mut scores: Vec<ProviderScore>,
    ) -> Vec<RoutingDecision> {
        let mut fallbacks = Vec::new();

        while let Some(fallback) = self.get_fallback(original, scores.clone()) {
            scores.retain(|score| {
                score.provider_id != fallback.provider_id || score.model_id != fallback.model_id
            });
            fallbacks.push(fallback);
        }

        fallbacks
    }
}

mod rand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    pub fn f64() -> f64 {
        let mut hasher = DefaultHasher::new();
        std::time::Instant::now().hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f64) / (u64::MAX as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_score(provider: &str, model: &str, total: f64) -> ProviderScore {
        ProviderScore {
            provider_id: provider.into(),
            model_id: model.into(),
            total_score: total,
            quality_score: total,
            cost_score: total,
            latency_score: total,
            availability_score: 1.0,
            estimated_cost_per_1k: 0.001,
            input_cost_per_1k: 0.0004,
            output_cost_per_1k: 0.0006,
            avg_latency_ms: 1000,
            availability: AvailabilityStatus::Available,
            policy_reason: None,
            score_breakdown: ScoreBreakdown {
                weighted_quality: total,
                weighted_cost: 0.0,
                weighted_latency: 0.0,
                weighted_availability: 0.0,
                weighted_policy: 0.0,
            },
        }
    }

    #[test]
    fn save_strategy_selects_lowest_cost_candidate_within_medium_quality_guard() {
        let mut highest_score = make_score("deepseek", "deepseek-v4-flash", 0.98);
        highest_score.quality_score = 0.98;
        highest_score.estimated_cost_per_1k = 0.0013;

        let mut cheapest_viable = make_score("siliconflow", "qwen2.5-7b", 0.94);
        cheapest_viable.quality_score = 0.83;
        cheapest_viable.estimated_cost_per_1k = 0.0002;

        let mut too_low_quality = make_score("ollama", "tiny-local", 0.90);
        too_low_quality.quality_score = 0.55;
        too_low_quality.estimated_cost_per_1k = 0.0;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![highest_score, cheapest_viable, too_low_quality],
                RoutingStrategy::Save,
                AgentTier::Medium,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "siliconflow");
        assert_eq!(decision.model_id, "qwen2.5-7b");
        assert!(decision.decision_reason.contains("lowest estimated cost"));
        assert_eq!(decision.candidates[0].provider_id, "siliconflow");
    }

    #[test]
    fn save_strategy_respects_tighter_quality_guard_for_reasoning_tasks() {
        let mut strongest = make_score("deepseek", "deepseek-v4-flash", 0.985);
        strongest.quality_score = 0.98;
        strongest.estimated_cost_per_1k = 0.0013;

        let mut cheaper_but_too_weak = make_score("mimo", "mimo-v2-flash", 0.956);
        cheaper_but_too_weak.quality_score = 0.82;
        cheaper_but_too_weak.estimated_cost_per_1k = 0.0004;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![cheaper_but_too_weak, strongest],
                RoutingStrategy::Save,
                AgentTier::Reasoning,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "deepseek");
    }

    #[test]
    fn save_strategy_prefers_higher_score_when_costs_are_nearly_equal() {
        let mut higher_score = make_score("deepseek", "deepseek-v4-flash", 0.985);
        higher_score.quality_score = 0.98;
        higher_score.estimated_cost_per_1k = 0.00042;

        let mut slightly_cheaper = make_score("mimo", "mimo-v2-flash", 0.956);
        slightly_cheaper.quality_score = 0.82;
        slightly_cheaper.estimated_cost_per_1k = 0.00040;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![slightly_cheaper, higher_score],
                RoutingStrategy::Save,
                AgentTier::Reasoning,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "deepseek");
        assert_eq!(decision.model_id, "deepseek-v4-flash");
        assert!(decision.decision_reason.contains("near-equal costs"));
    }

    #[test]
    fn fast_strategy_selects_lowest_latency_candidate_within_medium_quality_guard() {
        let mut highest_score = make_score("deepseek", "deepseek-v4-flash", 0.98);
        highest_score.quality_score = 0.98;
        highest_score.avg_latency_ms = 1500;

        let mut fastest_viable = make_score("mimo", "mimo-v2-flash", 0.94);
        fastest_viable.quality_score = 0.82;
        fastest_viable.avg_latency_ms = 600;

        let mut too_low_quality = make_score("tiny", "tiny-fast", 0.90);
        too_low_quality.quality_score = 0.50;
        too_low_quality.avg_latency_ms = 100;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![highest_score, fastest_viable, too_low_quality],
                RoutingStrategy::Fast,
                AgentTier::Medium,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "mimo");
        assert_eq!(decision.model_id, "mimo-v2-flash");
        assert!(decision.decision_reason.contains("lowest latency"));
    }

    #[test]
    fn fast_strategy_respects_tighter_quality_guard_for_reasoning_tasks() {
        let mut strongest = make_score("deepseek", "deepseek-v4-flash", 0.985);
        strongest.quality_score = 0.98;
        strongest.avg_latency_ms = 1500;

        let mut faster_but_too_weak = make_score("mimo", "mimo-v2-flash", 0.956);
        faster_but_too_weak.quality_score = 0.82;
        faster_but_too_weak.avg_latency_ms = 600;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![faster_but_too_weak, strongest],
                RoutingStrategy::Fast,
                AgentTier::Reasoning,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "deepseek");
    }

    #[test]
    fn fast_strategy_prefers_higher_score_when_latency_is_nearly_equal() {
        let mut higher_score = make_score("deepseek", "deepseek-v4-flash", 0.985);
        higher_score.quality_score = 0.98;
        higher_score.avg_latency_ms = 650;

        let mut slightly_faster = make_score("mimo", "mimo-v2-flash", 0.956);
        slightly_faster.quality_score = 0.82;
        slightly_faster.avg_latency_ms = 600;

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(
                vec![slightly_faster, higher_score],
                RoutingStrategy::Fast,
                AgentTier::Reasoning,
                &[],
            )
            .unwrap();

        assert_eq!(decision.provider_id, "deepseek");
        assert_eq!(decision.model_id, "deepseek-v4-flash");
        assert!(decision.decision_reason.contains("near-equal latency"));
    }

    #[test]
    fn selects_highest_score() {
        let scores = vec![
            make_score("openai", "gpt-4", 0.9),
            make_score("gemini", "gemini-flash", 0.6),
            make_score("anthropic", "claude-sonnet", 0.85),
        ];

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(scores, RoutingStrategy::Auto, AgentTier::Reasoning, &[])
            .unwrap();

        assert_eq!(decision.provider_id, "openai");
        assert!(!decision.is_fallback);
    }

    #[test]
    fn selected_route_explains_why_it_won() {
        let scores = vec![
            make_score("openai", "gpt-4", 0.9),
            make_score("gemini", "gemini-flash", 0.6),
        ];

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(scores, RoutingStrategy::Auto, AgentTier::Reasoning, &[])
            .unwrap();

        assert!(decision.decision_reason.contains("reasoning tier"));
        assert!(decision.decision_reason.contains("highest total score"));
        assert!(decision.decision_reason.contains("openai/gpt-4"));
    }

    #[test]
    fn excludes_unavailable() {
        let mut unavailable = make_score("openai", "gpt-4", 0.9);
        unavailable.availability = AvailabilityStatus::Unavailable;

        let scores = vec![unavailable, make_score("gemini", "gemini-flash", 0.6)];

        let selector = RouteSelector::new(RoutingConfig::default());
        let decision = selector
            .select(scores, RoutingStrategy::Auto, AgentTier::Reasoning, &[])
            .unwrap();

        assert_eq!(decision.provider_id, "gemini");
    }

    #[test]
    fn fallback_same_model_different_provider() {
        let scores = vec![
            make_score("openai", "gpt-4", 0.9),
            make_score("openrouter", "gpt-4", 0.7),
        ];

        let original = RoutingDecision {
            provider_id: "openai".into(),
            model_id: "gpt-4".into(),
            strategy: RoutingStrategy::Auto,
            agent_tier: AgentTier::Reasoning,
            candidates: scores.clone(),
            decision_time_ms: 0,
            is_fallback: false,
            fallback_reason: None,
            fallback_chain: vec![RouteAttempt {
                provider_id: "openai".into(),
                model_id: "gpt-4".into(),
                status: RouteAttemptStatus::Failed,
                error: Some("provider unavailable".into()),
            }],
            policy_reason: None,
            decision_reason: String::new(),
            excluded_candidates: Vec::new(),
        };

        let selector = RouteSelector::new(RoutingConfig::default());
        let fallback = selector.get_fallback(&original, scores).unwrap();

        assert!(fallback.is_fallback);
        assert_eq!(fallback.provider_id, "openrouter");
        assert_eq!(fallback.model_id, "gpt-4");
        assert_eq!(fallback.fallback_chain.len(), 1);
        assert_eq!(fallback.fallback_chain[0].provider_id, "openai");
        assert_eq!(
            fallback.fallback_chain[0].status,
            RouteAttemptStatus::Failed
        );
    }

    #[test]
    fn fallback_candidates_are_ordered_without_duplicates() {
        let mut same_model = make_score("openrouter", "gpt-4", 0.8);
        same_model.estimated_cost_per_1k = 0.02;
        let mut cheap_model = make_score("demo", "demo-echo", 0.6);
        cheap_model.estimated_cost_per_1k = 0.0;
        let mut other_model = make_score("local", "local-model", 0.7);
        other_model.estimated_cost_per_1k = 0.001;

        let original = RoutingDecision {
            provider_id: "openai".into(),
            model_id: "gpt-4".into(),
            strategy: RoutingStrategy::Auto,
            agent_tier: AgentTier::Medium,
            candidates: Vec::new(),
            decision_time_ms: 0,
            is_fallback: false,
            fallback_reason: None,
            fallback_chain: Vec::new(),
            policy_reason: None,
            decision_reason: String::new(),
            excluded_candidates: Vec::new(),
        };

        let selector = RouteSelector::new(RoutingConfig::default());
        let fallbacks =
            selector.get_fallback_candidates(&original, vec![cheap_model, same_model, other_model]);
        let routes: Vec<_> = fallbacks
            .iter()
            .map(|decision| (decision.provider_id.as_str(), decision.model_id.as_str()))
            .collect();

        assert_eq!(
            routes,
            vec![
                ("openrouter", "gpt-4"),
                ("demo", "demo-echo"),
                ("local", "local-model"),
            ]
        );
    }
}
