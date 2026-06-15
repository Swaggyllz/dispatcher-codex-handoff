use clap::{Parser, Subcommand};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_routing_config_uses_explicit_file() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-cli-config-test-{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            r#"
[tier_policies.simple]
cost_weight = 0.7
preferred_model_keywords = ["flash"]
"#,
        )
        .unwrap();

        let config = load_routing_config(Some(path.to_string_lossy().as_ref())).unwrap();
        std::fs::remove_file(&path).unwrap();

        let simple = config
            .tier_policies
            .get(&dispatcher_engine::AgentTier::Simple)
            .unwrap();
        assert_eq!(simple.cost_weight, Some(0.7));
        assert_eq!(simple.preferred_model_keywords, vec!["flash"]);
    }

    #[test]
    fn load_routing_config_source_preserves_explicit_persistence_path() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-cli-source-test-{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "fallback_enabled = false\n").unwrap();

        let loaded = load_routing_config_source(Some(path.to_string_lossy().as_ref())).unwrap();

        assert_eq!(loaded.path, path);
        assert!(!loaded.config.fallback_enabled);
        std::fs::remove_file(&path).unwrap();
    }
}

#[derive(Parser)]
#[command(name = "dispatch")]
#[command(about = "Dispatcher — LLM Intelligent Routing Engine", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动 HTTP server（OpenAI 兼容 API + Web Dashboard）
    Serve {
        /// 监听端口
        #[arg(short, long, default_value = "8787")]
        port: u16,

        /// Web dashboard 静态文件目录
        #[arg(short, long)]
        web_dir: Option<String>,

        /// Routing policy config file (TOML)
        #[arg(short, long)]
        config: Option<String>,
    },
    /// 路由单个请求（CLI 模式）
    Route {
        /// 模型名称
        #[arg(short, long)]
        model: String,

        /// 提示词
        #[arg(short, long)]
        prompt: String,

        /// 路由策略 (auto / save)
        #[arg(short, long, default_value = "auto")]
        strategy: String,

        /// Routing policy config file (TOML)
        #[arg(short, long)]
        config: Option<String>,
    },
    /// 显示配置信息
    Config {
        /// Routing policy config file (TOML)
        #[arg(short, long)]
        config: Option<String>,
    },
}

fn load_routing_config(path: Option<&str>) -> anyhow::Result<dispatcher_engine::RoutingConfig> {
    Ok(load_routing_config_source(path)?.config)
}

struct LoadedRoutingConfig {
    config: dispatcher_engine::RoutingConfig,
    path: std::path::PathBuf,
}

fn load_routing_config_source(path: Option<&str>) -> anyhow::Result<LoadedRoutingConfig> {
    if let Some(path) = path {
        let path = absolute_config_path(path)?;
        return Ok(LoadedRoutingConfig {
            config: dispatcher_engine::RoutingConfig::from_toml_file(&path)?,
            path,
        });
    }

    if let Ok(path) = std::env::var("DISPATCHER_CONFIG") {
        let path = absolute_config_path(path)?;
        return Ok(LoadedRoutingConfig {
            config: dispatcher_engine::RoutingConfig::from_toml_file(&path)?,
            path,
        });
    }

    let default_path = std::env::current_dir()?.join("dispatcher.toml");
    let mut candidates = vec![default_path.clone()];
    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("dispatcher").join("config.toml"));
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(LoadedRoutingConfig {
                config: dispatcher_engine::RoutingConfig::from_toml_file(&candidate)?,
                path: candidate,
            });
        }
    }

    Ok(LoadedRoutingConfig {
        config: dispatcher_engine::RoutingConfig::default(),
        path: default_path,
    })
}

fn absolute_config_path(path: impl AsRef<std::path::Path>) -> anyhow::Result<std::path::PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "dispatch=info,dispatcher_server=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            port,
            web_dir,
            config,
        } => {
            let loaded = load_routing_config_source(config.as_deref())?;
            dispatcher_server::run_with_config_path(
                port,
                web_dir,
                loaded.config,
                Some(loaded.path),
            )
            .await?;
        }
        Commands::Route {
            model: _model,
            prompt,
            strategy,
            config,
        } => {
            let strategy = match strategy.as_str() {
                "save" => dispatcher_engine::RoutingStrategy::Save,
                "fast" => dispatcher_engine::RoutingStrategy::Fast,
                _ => dispatcher_engine::RoutingStrategy::Auto,
            };

            let request = dispatcher_engine::ModelRequest {
                model: "auto".into(),
                messages: vec![dispatcher_engine::Message {
                    role: "user".into(),
                    content: dispatcher_engine::MessageContent::Text(prompt),
                }],
                temperature: None,
                max_tokens: None,
                stream: false,
                tools: None,
                extra: Default::default(),
            };

            let registry = dispatcher_providers::ProviderRegistry::from_env();
            let routing_config = load_routing_config(config.as_deref())?;
            let engine = dispatcher_engine::RoutingEngine::new(routing_config);

            let capabilities = registry.capabilities().to_vec();
            match engine.route(&request, &capabilities, strategy).await {
                Some(decision) => {
                    println!("路由决策:");
                    println!("  Provider: {}", decision.provider_id);
                    println!("  Model:    {}", decision.model_id);
                    println!("  Strategy: {:?}", decision.strategy);
                    println!("  Fallback: {}", decision.is_fallback);
                    println!(
                        "  Score:    {:.3}",
                        decision
                            .candidates
                            .first()
                            .map(|s| s.total_score)
                            .unwrap_or(0.0)
                    );
                    println!("  Time:     {}ms", decision.decision_time_ms);
                    println!();
                    println!("候选评分:");
                    for candidate in &decision.candidates {
                        println!(
                            "  {} / {} — total={:.3} quality={:.3} cost={:.3} latency={:.3}",
                            candidate.provider_id,
                            candidate.model_id,
                            candidate.total_score,
                            candidate.quality_score,
                            candidate.cost_score,
                            candidate.latency_score
                        );
                    }

                    // 实际发送请求
                    println!();
                    println!("正在请求...");
                    if let Some(provider) = registry.get(&decision.provider_id) {
                        match provider.chat_completion(&request, &decision.model_id).await {
                            Ok(response) => {
                                println!();
                                println!(
                                    "{}",
                                    response
                                        .choices
                                        .first()
                                        .map(|c| c.message.content.as_str())
                                        .unwrap_or("")
                                );
                                println!();
                                println!(
                                    "Tokens: {} in / {} out | Latency: {}ms",
                                    response.usage.prompt_tokens,
                                    response.usage.completion_tokens,
                                    response.latency_ms
                                );
                            }
                            Err(e) => {
                                eprintln!("请求失败: {}", e);
                            }
                        }
                    }
                }
                None => {
                    eprintln!("没有可用的 provider");
                }
            }
        }
        Commands::Config { config } => {
            let routing_config = load_routing_config(config.as_deref())?;
            let registry = dispatcher_providers::ProviderRegistry::from_env();
            let providers = registry.list_providers();
            let capabilities = registry.capabilities();

            println!("Dispatcher v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!(
                "Routing policies: {} tier override(s)",
                routing_config.tier_policies.len()
            );
            println!();
            println!("已注册的 Providers:");
            for provider_id in &providers {
                if let Some(cap) = capabilities.iter().find(|c| &c.provider_id == provider_id) {
                    println!("  {} ({})", cap.provider_name, cap.provider_id);
                    println!("    URL: {}", cap.base_url);
                    println!("    模型数: {}", cap.supported_models.len());
                    for model in &cap.supported_models {
                        println!(
                            "      - {} (质量: {:.2}, 成本: ${:.6}/1K, 延迟: {}ms)",
                            model.model_id,
                            model.quality_score,
                            model.input_cost_per_1k,
                            model.avg_latency_ms
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
