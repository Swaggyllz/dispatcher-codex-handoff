use dispatcher_engine::types::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::anthropic::AnthropicProvider;
use crate::deepseek::DeepSeekProvider;
use crate::demo::DemoProvider;
use crate::gemini::GeminiProvider;
use crate::mimo::MiMoProvider;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAIProvider;
use crate::openrouter::OpenRouterProvider;
use crate::siliconflow::SiliconFlowProvider;

/// Provider 注册中心 — 管理所有已注册的 LLM provider
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    capabilities: Vec<ProviderCapability>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            capabilities: Vec::new(),
        }
    }

    /// 从环境变量创建默认 provider 集合
    pub fn from_env() -> Self {
        let mut registry = Self::new();

        if std::env::var("DISPATCHER_DEMO_PROVIDER").as_deref() != Ok("0") {
            registry.register(Arc::new(DemoProvider::new()));
        }

        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(AnthropicProvider::new(api_key)));
            }
        }

        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(OpenAIProvider::new(api_key)));
            }
        }

        if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(GeminiProvider::new(api_key)));
            }
        }

        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(OpenRouterProvider::new(api_key)));
            }
        }

        // Ollama 不需要 API key，默认注册
        registry.register(Arc::new(OllamaProvider::new(
            std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".into()),
        )));

        if let Ok(api_key) = std::env::var("SILICONFLOW_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(SiliconFlowProvider::new(api_key)));
            }
        }

        if let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY") {
            if !api_key.is_empty() {
                registry.register(Arc::new(DeepSeekProvider::new(api_key)));
            }
        }

        if let Ok(api_key) =
            std::env::var("MIMO_API_KEY").or_else(|_| std::env::var("XIAOMIMIMO_API_KEY"))
        {
            if !api_key.is_empty() {
                registry.register(Arc::new(MiMoProvider::new(api_key)));
            }
        }

        registry.apply_metadata_overrides();

        registry
    }

    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        let id = provider.provider_id().to_string();
        let cap = provider.capability().clone();
        self.providers.insert(id, provider);
        self.capabilities.push(cap);
    }

    pub fn get(&self, provider_id: &str) -> Option<&Arc<dyn Provider>> {
        self.providers.get(provider_id)
    }

    pub fn capabilities(&self) -> &[ProviderCapability] {
        &self.capabilities
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.providers.contains_key(provider_id)
    }

    fn apply_metadata_overrides(&mut self) {
        if let Err(error) = crate::metadata::apply_default_metadata(&mut self.capabilities) {
            tracing::warn!("Failed to apply default provider metadata: {error}");
        }

        if let Ok(path) = std::env::var("DISPATCHER_PROVIDER_METADATA") {
            if !path.trim().is_empty() {
                if let Err(error) =
                    crate::metadata::apply_metadata_file(&mut self.capabilities, Path::new(&path))
                {
                    tracing::warn!("Failed to apply provider metadata from {path}: {error}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_accepts_xiaomi_mimo_api_key_alias() {
        std::env::remove_var("MIMO_API_KEY");
        std::env::set_var("XIAOMIMIMO_API_KEY", "test-key");

        let registry = ProviderRegistry::from_env();

        std::env::remove_var("XIAOMIMIMO_API_KEY");
        assert!(registry.has_provider("mimo"));
    }

    #[test]
    fn from_env_registers_demo_provider_by_default() {
        std::env::remove_var("DISPATCHER_DEMO_PROVIDER");

        let registry = ProviderRegistry::from_env();

        assert!(registry.has_provider("demo"));
    }

    #[test]
    fn from_env_can_override_provider_model_metadata_from_file() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-provider-metadata-{}.toml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
[[providers]]
id = "deepseek"
name = "DeepSeek Custom"
supports_streaming = false
supports_tools = true
supports_vision = true
max_context_length = 131072

[[providers.models]]
id = "deepseek-v4-flash"
name = "DeepSeek V4 Flash Custom"
input_cost_per_1k = 0.001
output_cost_per_1k = 0.002
max_tokens = 131072
quality_score = 0.91
avg_latency_ms = 777
"#,
        )
        .unwrap();

        std::env::set_var("DISPATCHER_DEMO_PROVIDER", "0");
        std::env::set_var("DEEPSEEK_API_KEY", "test-key");
        std::env::set_var("DISPATCHER_PROVIDER_METADATA", &path);

        let registry = ProviderRegistry::from_env();
        let capability = registry
            .capabilities()
            .iter()
            .find(|capability| capability.provider_id == "deepseek")
            .unwrap();
        let model = capability
            .supported_models
            .iter()
            .find(|model| model.model_id == "deepseek-v4-flash")
            .unwrap();

        std::env::remove_var("DISPATCHER_DEMO_PROVIDER");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("DISPATCHER_PROVIDER_METADATA");
        std::fs::remove_file(path).unwrap();

        assert_eq!(capability.provider_name, "DeepSeek Custom");
        assert!(!capability.supports_streaming);
        assert!(capability.supports_vision);
        assert_eq!(capability.max_context_length, 131072);
        assert_eq!(model.display_name, "DeepSeek V4 Flash Custom");
        assert!((model.input_cost_per_1k - 0.001).abs() < f64::EPSILON);
        assert!((model.output_cost_per_1k - 0.002).abs() < f64::EPSILON);
        assert_eq!(model.max_tokens, 131072);
        assert_eq!(model.avg_latency_ms, 777);
    }
}
