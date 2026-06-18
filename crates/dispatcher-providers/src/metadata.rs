use dispatcher_engine::types::{HandoffCertification, ModelInfo, ProviderCapability};
use serde::Deserialize;
use std::path::Path;

const DEFAULT_METADATA: &str = include_str!("../provider-models.toml");

#[derive(Debug, Deserialize)]
struct MetadataFile {
    #[serde(default)]
    providers: Vec<ProviderMetadata>,
}

#[derive(Debug, Deserialize)]
struct ProviderMetadata {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    requires_api_key: Option<bool>,
    supports_streaming: Option<bool>,
    supports_tools: Option<bool>,
    supports_vision: Option<bool>,
    max_context_length: Option<usize>,
    #[serde(default)]
    models: Vec<ModelMetadata>,
}

#[derive(Debug, Deserialize)]
struct ModelMetadata {
    id: String,
    name: Option<String>,
    input_cost_per_1k: Option<f64>,
    output_cost_per_1k: Option<f64>,
    pricing_source: Option<String>,
    pricing_updated_at: Option<String>,
    supports_streaming: Option<bool>,
    supports_tools: Option<bool>,
    supports_vision: Option<bool>,
    max_tokens: Option<u32>,
    quality_score: Option<f64>,
    avg_latency_ms: Option<u64>,
    handoff_certification: Option<HandoffCertification>,
}

pub fn apply_default_metadata(capabilities: &mut [ProviderCapability]) -> anyhow::Result<()> {
    let metadata = parse_metadata(DEFAULT_METADATA)?;
    apply_metadata(capabilities, &metadata);
    Ok(())
}

pub fn apply_metadata_file(
    capabilities: &mut [ProviderCapability],
    path: &Path,
) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let metadata = parse_metadata(&content)?;
    apply_metadata(capabilities, &metadata);
    Ok(())
}

fn parse_metadata(content: &str) -> anyhow::Result<MetadataFile> {
    let metadata: MetadataFile = toml::from_str(content)?;
    validate_metadata(&metadata)?;
    Ok(metadata)
}

fn validate_metadata(metadata: &MetadataFile) -> anyhow::Result<()> {
    for provider in &metadata.providers {
        anyhow::ensure!(
            !provider.id.trim().is_empty(),
            "provider id must not be empty"
        );
        if let Some(max_context_length) = provider.max_context_length {
            anyhow::ensure!(
                max_context_length > 0,
                "max_context_length must be positive"
            );
        }
        for model in &provider.models {
            anyhow::ensure!(!model.id.trim().is_empty(), "model id must not be empty");
            validate_non_negative(model.input_cost_per_1k, "input_cost_per_1k")?;
            validate_non_negative(model.output_cost_per_1k, "output_cost_per_1k")?;
            validate_non_empty(model.pricing_source.as_deref(), "pricing_source")?;
            validate_date(model.pricing_updated_at.as_deref())?;
            validate_quality(model.quality_score)?;
            validate_handoff_certification(model.handoff_certification.as_ref())?;
            if let Some(max_tokens) = model.max_tokens {
                anyhow::ensure!(max_tokens > 0, "max_tokens must be positive");
            }
        }
    }
    Ok(())
}

fn validate_non_negative(value: Option<f64>, field: &str) -> anyhow::Result<()> {
    if let Some(value) = value {
        anyhow::ensure!(
            value.is_finite() && value >= 0.0,
            "{field} must be non-negative"
        );
    }
    Ok(())
}

fn validate_quality(value: Option<f64>) -> anyhow::Result<()> {
    if let Some(value) = value {
        anyhow::ensure!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "quality_score must be between 0 and 1"
        );
    }
    Ok(())
}

fn validate_non_empty(value: Option<&str>, field: &str) -> anyhow::Result<()> {
    if let Some(value) = value {
        anyhow::ensure!(!value.trim().is_empty(), "{field} must not be empty");
    }
    Ok(())
}

fn validate_date(value: Option<&str>) -> anyhow::Result<()> {
    if let Some(value) = value {
        anyhow::ensure!(
            value.len() == 10
                && value.as_bytes()[4] == b'-'
                && value.as_bytes()[7] == b'-'
                && value
                    .chars()
                    .enumerate()
                    .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()),
            "pricing_updated_at must use YYYY-MM-DD"
        );
    }
    Ok(())
}

fn validate_handoff_certification(value: Option<&HandoffCertification>) -> anyhow::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };

    anyhow::ensure!(
        !value.labels.is_empty(),
        "handoff_certification.labels must not be empty"
    );
    validate_non_empty(value.eval_set.as_deref(), "handoff_certification.eval_set")?;
    validate_date(value.evaluated_at.as_deref())?;
    Ok(())
}

fn apply_metadata(capabilities: &mut [ProviderCapability], metadata: &MetadataFile) {
    for override_provider in &metadata.providers {
        let Some(capability) = capabilities
            .iter_mut()
            .find(|capability| capability.provider_id == override_provider.id)
        else {
            continue;
        };

        apply_provider_metadata(capability, override_provider);
    }
}

fn apply_provider_metadata(
    capability: &mut ProviderCapability,
    override_provider: &ProviderMetadata,
) {
    if let Some(name) = &override_provider.name {
        capability.provider_name = name.clone();
    }
    if let Some(base_url) = &override_provider.base_url {
        capability.base_url = base_url.clone();
    }
    if let Some(requires_api_key) = override_provider.requires_api_key {
        capability.requires_api_key = requires_api_key;
    }
    if let Some(supports_streaming) = override_provider.supports_streaming {
        capability.supports_streaming = supports_streaming;
    }
    if let Some(supports_tools) = override_provider.supports_tools {
        capability.supports_tools = supports_tools;
    }
    if let Some(supports_vision) = override_provider.supports_vision {
        capability.supports_vision = supports_vision;
    }
    if let Some(max_context_length) = override_provider.max_context_length {
        capability.max_context_length = max_context_length;
    }

    for override_model in &override_provider.models {
        if let Some(model) = capability
            .supported_models
            .iter_mut()
            .find(|model| model.model_id == override_model.id)
        {
            apply_model_metadata(model, override_model);
        } else {
            capability
                .supported_models
                .push(model_from_metadata(override_model));
        }
    }
}

fn apply_model_metadata(model: &mut ModelInfo, override_model: &ModelMetadata) {
    if let Some(name) = &override_model.name {
        model.display_name = name.clone();
    }
    if let Some(input_cost_per_1k) = override_model.input_cost_per_1k {
        model.input_cost_per_1k = input_cost_per_1k;
    }
    if let Some(output_cost_per_1k) = override_model.output_cost_per_1k {
        model.output_cost_per_1k = output_cost_per_1k;
    }
    if let Some(pricing_source) = &override_model.pricing_source {
        model.pricing_source = Some(pricing_source.clone());
    }
    if let Some(pricing_updated_at) = &override_model.pricing_updated_at {
        model.pricing_updated_at = Some(pricing_updated_at.clone());
    }
    if let Some(supports_streaming) = override_model.supports_streaming {
        model.supports_streaming = Some(supports_streaming);
    }
    if let Some(supports_tools) = override_model.supports_tools {
        model.supports_tools = Some(supports_tools);
    }
    if let Some(supports_vision) = override_model.supports_vision {
        model.supports_vision = Some(supports_vision);
    }
    if let Some(max_tokens) = override_model.max_tokens {
        model.max_tokens = max_tokens;
    }
    if let Some(quality_score) = override_model.quality_score {
        model.quality_score = quality_score;
    }
    if let Some(avg_latency_ms) = override_model.avg_latency_ms {
        model.avg_latency_ms = avg_latency_ms;
    }
    if let Some(handoff_certification) = &override_model.handoff_certification {
        model.handoff_certification = handoff_certification.clone();
    }
}

fn model_from_metadata(model: &ModelMetadata) -> ModelInfo {
    ModelInfo {
        model_id: model.id.clone(),
        display_name: model.name.clone().unwrap_or_else(|| model.id.clone()),
        input_cost_per_1k: model.input_cost_per_1k.unwrap_or(0.0),
        output_cost_per_1k: model.output_cost_per_1k.unwrap_or(0.0),
        pricing_source: model.pricing_source.clone(),
        pricing_updated_at: model.pricing_updated_at.clone(),
        supports_streaming: model.supports_streaming,
        supports_tools: model.supports_tools,
        supports_vision: model.supports_vision,
        max_tokens: model.max_tokens.unwrap_or(4096),
        quality_score: model.quality_score.unwrap_or(0.5),
        avg_latency_ms: model.avg_latency_ms.unwrap_or(1000),
        handoff_certification: model.handoff_certification.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatcher_engine::types::HandoffCertificationLabel;

    fn test_capability() -> ProviderCapability {
        ProviderCapability {
            provider_id: "alpha".into(),
            provider_name: "Alpha".into(),
            supported_models: vec![ModelInfo {
                model_id: "alpha-fast".into(),
                display_name: "Alpha Fast".into(),
                input_cost_per_1k: 0.01,
                output_cost_per_1k: 0.02,
                pricing_source: None,
                pricing_updated_at: None,
                supports_streaming: None,
                supports_tools: None,
                supports_vision: None,
                max_tokens: 4096,
                quality_score: 0.7,
                avg_latency_ms: 800,
                handoff_certification: HandoffCertification::default(),
            }],
            base_url: "https://example.test".into(),
            requires_api_key: true,
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            max_context_length: 8192,
        }
    }

    #[test]
    fn metadata_file_applies_model_pricing_source_and_capability_overrides() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-model-metadata-{}.toml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
[[providers]]
id = "alpha"

[[providers.models]]
id = "alpha-fast"
input_cost_per_1k = 0.003
output_cost_per_1k = 0.009
pricing_source = "https://example.test/pricing"
pricing_updated_at = "2026-06-08"
supports_tools = false
supports_vision = false
"#,
        )
        .unwrap();

        let mut capabilities = vec![test_capability()];
        apply_metadata_file(&mut capabilities, &path).unwrap();
        std::fs::remove_file(path).unwrap();

        let serialized = serde_json::to_value(&capabilities[0].supported_models[0]).unwrap();
        assert_eq!(serialized["pricing_source"], "https://example.test/pricing");
        assert_eq!(serialized["pricing_updated_at"], "2026-06-08");
        assert_eq!(serialized["supports_tools"], false);
        assert_eq!(serialized["supports_vision"], false);
    }

    #[test]
    fn metadata_file_applies_model_handoff_certification() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-model-certification-{}.toml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
[[providers]]
id = "alpha"

[[providers.models]]
id = "alpha-fast"
handoff_certification = { labels = ["handoff_text_only", "handoff_code_patch"], eval_set = "dispatcher-handoff-v0.3.0-fixtures", evaluated_at = "2026-06-18", notes = "fixture-backed test profile" }
"#,
        )
        .unwrap();

        let mut capabilities = vec![test_capability()];
        apply_metadata_file(&mut capabilities, &path).unwrap();
        std::fs::remove_file(path).unwrap();

        let certification = &capabilities[0].supported_models[0].handoff_certification;
        assert!(certification.is_certified());
        assert_eq!(
            certification.labels,
            vec![
                HandoffCertificationLabel::HandoffTextOnly,
                HandoffCertificationLabel::HandoffCodePatch
            ]
        );
        assert_eq!(
            certification.eval_set.as_deref(),
            Some("dispatcher-handoff-v0.3.0-fixtures")
        );
    }
}
