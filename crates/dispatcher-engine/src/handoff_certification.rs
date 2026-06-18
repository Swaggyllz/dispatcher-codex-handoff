use crate::{ExcludedCandidate, ModelInfo, ProviderCapability, RequestFeatures, TaskType};
use serde::{Deserialize, Serialize};

const BUILTIN_FIXTURES: &str = include_str!("handoff_eval_fixtures.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffCertificationLabel {
    HandoffTextOnly,
    HandoffCodePatch,
    HandoffToolCapable,
    HandoffLongContext,
    NotCertified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffCertification {
    #[serde(default = "default_handoff_labels")]
    pub labels: Vec<HandoffCertificationLabel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEvalFixture {
    pub id: String,
    pub required_label: HandoffCertificationLabel,
    pub prompt: String,
    pub success_criteria: Vec<String>,
}

impl Default for HandoffCertification {
    fn default() -> Self {
        Self {
            labels: default_handoff_labels(),
            eval_set: None,
            evaluated_at: None,
            notes: None,
        }
    }
}

impl HandoffCertification {
    pub fn is_certified(&self) -> bool {
        self.labels
            .iter()
            .any(|label| *label != HandoffCertificationLabel::NotCertified)
    }

    pub fn satisfies(&self, required: HandoffCertificationLabel) -> bool {
        if !self.is_certified() {
            return false;
        }

        match required {
            HandoffCertificationLabel::HandoffTextOnly => self.labels.iter().any(|label| {
                matches!(
                    label,
                    HandoffCertificationLabel::HandoffTextOnly
                        | HandoffCertificationLabel::HandoffCodePatch
                        | HandoffCertificationLabel::HandoffToolCapable
                        | HandoffCertificationLabel::HandoffLongContext
                )
            }),
            _ => self.labels.contains(&required),
        }
    }
}

fn default_handoff_labels() -> Vec<HandoffCertificationLabel> {
    vec![HandoffCertificationLabel::NotCertified]
}

pub fn builtin_handoff_eval_fixtures() -> anyhow::Result<Vec<HandoffEvalFixture>> {
    Ok(serde_json::from_str(BUILTIN_FIXTURES)?)
}

pub fn required_handoff_label(features: &RequestFeatures) -> HandoffCertificationLabel {
    if features.has_tools {
        HandoffCertificationLabel::HandoffToolCapable
    } else if features.is_long_context {
        HandoffCertificationLabel::HandoffLongContext
    } else if matches!(features.task_type, TaskType::Code) {
        HandoffCertificationLabel::HandoffCodePatch
    } else {
        HandoffCertificationLabel::HandoffTextOnly
    }
}

pub fn filter_handoff_eligible_capabilities(
    capabilities: &[ProviderCapability],
    features: &RequestFeatures,
) -> (Vec<ProviderCapability>, Vec<ExcludedCandidate>) {
    let required = required_handoff_label(features);
    let mut eligible_capabilities = Vec::new();
    let mut excluded = Vec::new();

    for capability in capabilities {
        let mut eligible = capability.clone();
        eligible.supported_models.retain(|model| {
            if let Some(reason) = handoff_model_rejection_reason(model, required) {
                excluded.push(ExcludedCandidate {
                    provider_id: capability.provider_id.clone(),
                    model_id: Some(model.model_id.clone()),
                    reason,
                });
                false
            } else {
                true
            }
        });

        if !eligible.supported_models.is_empty() {
            eligible_capabilities.push(eligible);
        }
    }

    (eligible_capabilities, excluded)
}

fn handoff_model_rejection_reason(
    model: &ModelInfo,
    required: HandoffCertificationLabel,
) -> Option<String> {
    if !model.handoff_certification.is_certified() {
        return Some("handoff worker not certified".into());
    }
    if !model.handoff_certification.satisfies(required) {
        return Some(format!(
            "handoff worker missing {} certification",
            handoff_label_name(required)
        ));
    }
    None
}

pub fn handoff_label_name(label: HandoffCertificationLabel) -> &'static str {
    match label {
        HandoffCertificationLabel::HandoffTextOnly => "handoff_text_only",
        HandoffCertificationLabel::HandoffCodePatch => "handoff_code_patch",
        HandoffCertificationLabel::HandoffToolCapable => "handoff_tool_capable",
        HandoffCertificationLabel::HandoffLongContext => "handoff_long_context",
        HandoffCertificationLabel::NotCertified => "not_certified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentTier, Message, MessageContent, ModelInfo, ModelRequest, ProviderCapability,
        RequestAnalyzer, TaskType,
    };

    fn model(labels: Vec<HandoffCertificationLabel>, max_tokens: u32) -> ModelInfo {
        ModelInfo {
            model_id: "worker-model".into(),
            display_name: "Worker Model".into(),
            input_cost_per_1k: 0.001,
            output_cost_per_1k: 0.002,
            pricing_source: None,
            pricing_updated_at: None,
            supports_streaming: Some(true),
            supports_tools: Some(true),
            supports_vision: Some(false),
            max_tokens,
            quality_score: 0.85,
            avg_latency_ms: 900,
            handoff_certification: HandoffCertification {
                labels,
                eval_set: Some("dispatcher-handoff-v0.3.0-fixtures".into()),
                evaluated_at: Some("2026-06-18".into()),
                notes: Some("unit test profile".into()),
            },
        }
    }

    fn capability(model: ModelInfo) -> ProviderCapability {
        ProviderCapability {
            provider_id: "worker".into(),
            provider_name: "Worker".into(),
            supported_models: vec![model],
            base_url: "https://example.test".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: false,
            max_context_length: 128_000,
        }
    }

    fn request(text: &str) -> ModelRequest {
        ModelRequest {
            model: "auto".into(),
            messages: vec![Message {
                role: "user".into(),
                content: MessageContent::Text(text.into()),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn default_certification_is_not_certified() {
        let certification = HandoffCertification::default();
        assert_eq!(
            certification.labels,
            vec![HandoffCertificationLabel::NotCertified]
        );
        assert!(!certification.is_certified());
    }

    #[test]
    fn builtin_fixtures_parse_and_cover_supported_labels() {
        let fixtures = builtin_handoff_eval_fixtures().unwrap();
        let labels = fixtures
            .iter()
            .map(|fixture| fixture.required_label)
            .collect::<std::collections::HashSet<_>>();

        assert!(labels.contains(&HandoffCertificationLabel::HandoffTextOnly));
        assert!(labels.contains(&HandoffCertificationLabel::HandoffCodePatch));
        assert!(labels.contains(&HandoffCertificationLabel::HandoffToolCapable));
        assert!(labels.contains(&HandoffCertificationLabel::HandoffLongContext));
    }

    #[test]
    fn code_handoff_requires_code_patch_certification() {
        let features = RequestAnalyzer::analyze(&request(
            "Implement the remaining Rust API and return the patch.",
        ));

        assert_eq!(features.task_type, TaskType::Code);
        assert_eq!(
            required_handoff_label(&features),
            HandoffCertificationLabel::HandoffCodePatch
        );
    }

    #[test]
    fn filter_excludes_uncertified_handoff_workers() {
        let features = RequestAnalyzer::analyze(&request("Summarize the handoff package."));
        let uncertified = capability(ModelInfo {
            handoff_certification: HandoffCertification::default(),
            ..model(vec![HandoffCertificationLabel::HandoffTextOnly], 8192)
        });

        let (eligible, excluded) = filter_handoff_eligible_capabilities(&[uncertified], &features);

        assert!(eligible.is_empty());
        assert_eq!(excluded[0].reason, "handoff worker not certified");
    }

    #[test]
    fn filter_keeps_matching_certified_worker() {
        let features = RequestAnalyzer::analyze(&request("Summarize the handoff package."));
        let certified = capability(model(
            vec![HandoffCertificationLabel::HandoffTextOnly],
            8192,
        ));

        let (eligible, excluded) = filter_handoff_eligible_capabilities(&[certified], &features);

        assert_eq!(eligible.len(), 1);
        assert!(excluded.is_empty());
    }

    #[test]
    fn long_context_handoff_requires_long_context_certification() {
        let long_request = request(&"Summarize this handoff package. ".repeat(9_000));
        let features = RequestAnalyzer::analyze(&long_request);

        assert_eq!(features.agent_tier, AgentTier::Reasoning);
        assert_eq!(
            required_handoff_label(&features),
            HandoffCertificationLabel::HandoffLongContext
        );
    }
}
