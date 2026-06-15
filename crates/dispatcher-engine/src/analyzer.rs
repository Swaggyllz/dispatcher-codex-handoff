use crate::types::*;

/// 分析请求，提取特征用于路由决策
pub struct RequestAnalyzer;

impl RequestAnalyzer {
    pub fn analyze(request: &ModelRequest) -> RequestFeatures {
        let estimated_tokens = Self::estimate_tokens(request);
        let has_tools = request.tools.as_ref().is_some_and(|t| !t.is_empty());
        let has_images = Self::has_images(request);
        let routing_intent = Self::routing_intent_request(request);
        let task_type = Self::detect_task_type(&routing_intent);
        let complexity_score = Self::calculate_complexity(&routing_intent, &task_type);
        let agent_tier = Self::classify_agent_tier(&routing_intent, &task_type, complexity_score);
        let is_long_context = estimated_tokens > 32_000;

        RequestFeatures {
            estimated_tokens,
            has_tools,
            has_images,
            is_streaming: request.stream,
            complexity_score,
            task_type,
            agent_tier,
            is_long_context,
        }
    }

    fn routing_intent_request(request: &ModelRequest) -> ModelRequest {
        let messages = request
            .messages
            .iter()
            .rev()
            .find(|message| Self::is_real_user_intent(message))
            .cloned()
            .into_iter()
            .collect();

        ModelRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: request.stream,
            tools: None,
            extra: Default::default(),
        }
    }

    pub fn latest_user_intent_text(request: &ModelRequest) -> Option<String> {
        request
            .messages
            .iter()
            .rev()
            .find(|message| Self::is_real_user_intent(message))
            .map(Self::message_text)
    }

    fn is_real_user_intent(message: &Message) -> bool {
        message.role == "user" && !Self::is_synthetic_user_context(&Self::message_text(message))
    }

    fn is_synthetic_user_context(text: &str) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.starts_with("[Tool result") {
            return true;
        }

        [
            "environment_context",
            "permissions instructions",
            "app-context",
            "collaboration_mode",
            "personality_spec",
            "apps_instructions",
            "skills_instructions",
            "plugins_instructions",
        ]
        .iter()
        .any(|tag| {
            trimmed.starts_with(&format!("<{tag}")) && trimmed.ends_with(&format!("</{tag}>"))
        })
    }

    fn message_text(message: &Message) -> String {
        match &message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::MultiPart(parts) => parts
                .iter()
                .filter_map(|part| part.text.as_deref())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    /// 估算输入 token 数（粗略估计：中文 ~1.5 char/token，英文 ~4 char/token）
    fn estimate_tokens(request: &ModelRequest) -> usize {
        let mut total_chars = 0usize;
        for msg in &request.messages {
            match &msg.content {
                MessageContent::Text(t) => total_chars += t.chars().count(),
                MessageContent::MultiPart(parts) => {
                    for part in parts {
                        if let Some(ref t) = part.text {
                            total_chars += t.chars().count();
                        }
                        if part.image_url.is_some() {
                            total_chars += 500; // 每张图约等于 500 tokens
                        }
                    }
                }
            }
        }
        // 粗略估算：中英文混合，按 2.5 chars/token
        (total_chars as f64 / 2.5) as usize + 100 // +100 system overhead
    }

    fn has_images(request: &ModelRequest) -> bool {
        request.messages.iter().any(|msg| match &msg.content {
            MessageContent::MultiPart(parts) => parts.iter().any(|p| p.image_url.is_some()),
            _ => false,
        })
    }

    fn detect_task_type(request: &ModelRequest) -> TaskType {
        let all_text = Self::all_text(request);
        let lower = all_text.to_lowercase();

        // 代码相关关键词
        let code_keywords = [
            "function",
            "class",
            "code",
            "bug",
            "error",
            "implement",
            "refactor",
            "debug",
            "compile",
            "runtime",
            "syntax",
            "api",
            "endpoint",
            "query",
            "select",
            "import",
            "export",
            "const",
            "let",
            "var",
            "async",
            "await",
            "fn",
            "impl",
            "struct",
            "pub",
            "def",
            "编写",
            "代码",
            "函数",
            "实现",
        ];
        // 分析相关关键词
        let analysis_keywords = [
            "analyze",
            "analysis",
            "explain",
            "why",
            "compare",
            "evaluate",
            "assess",
            "review",
            "audit",
            "investigate",
            "分析",
            "解释",
            "比较",
            "评估",
        ];
        // 创意相关关键词
        let creative_keywords = [
            "story", "poem", "write", "creative", "imagine", "scenario", "novel", "fiction",
            "故事", "诗", "创意", "写", "想象",
        ];
        // 翻译相关关键词
        let translation_keywords = [
            "translate",
            "translation",
            "chinese",
            "english",
            "japanese",
            "翻译",
            "中文",
            "英文",
            "日文",
        ];
        // 总结相关关键词
        let summary_keywords = [
            "summarize",
            "summary",
            "tldr",
            "brief",
            "concise",
            "总结",
            "摘要",
            "概括",
            "简述",
        ];

        let count =
            |keywords: &[&str]| -> usize { keywords.iter().filter(|k| lower.contains(*k)).count() };

        let scores = [
            (TaskType::Code, count(&code_keywords)),
            (TaskType::Analysis, count(&analysis_keywords)),
            (TaskType::Creative, count(&creative_keywords)),
            (TaskType::Translation, count(&translation_keywords)),
            (TaskType::Summarization, count(&summary_keywords)),
        ];

        scores
            .iter()
            .max_by_key(|(_, c)| *c)
            .filter(|(_, c)| *c > 0)
            .map(|(t, _)| *t)
            .unwrap_or(TaskType::Chat)
    }

    fn classify_agent_tier(
        request: &ModelRequest,
        task_type: &TaskType,
        complexity_score: f64,
    ) -> AgentTier {
        let text = Self::all_text(request);
        let lower = text.to_lowercase();
        let estimated_tokens = Self::estimate_tokens(request);
        let has_tools = request.tools.as_ref().is_some_and(|t| !t.is_empty());

        let complex_markers = [
            "sub agent",
            "sub-agent",
            "agent",
            "parallel",
            "concurrent",
            "orchestrate",
            "workflow",
            "multi-workflow",
            "delegate",
            "并行",
            "子 agent",
            "子agent",
            "编排",
            "委派",
            "多个团队",
            "前端、后端",
            "前端 后端",
            "多工作流",
        ];
        if complex_markers.iter().any(|marker| lower.contains(marker)) {
            return AgentTier::Complex;
        }

        let reasoning_markers = [
            "architecture",
            "architect",
            "security",
            "audit",
            "review",
            "refactor",
            "debug",
            "investigate",
            "analyze",
            "analysis",
            "multi-file",
            "plan",
            "from scratch",
            "full project",
            "service",
            "test suite",
            "架构",
            "安全",
            "审查",
            "重构",
            "调试",
            "排查",
            "分析",
            "方案",
            "多文件",
            "整个项目",
            "系统",
            "风险",
            "复杂",
            "错误处理",
            "从零",
            "完整项目",
            "项目",
            "服务",
            "配置加载",
            "测试",
        ];
        if estimated_tokens > 8_000
            || complexity_score >= 0.35
            || reasoning_markers
                .iter()
                .filter(|marker| lower.contains(**marker))
                .count()
                >= 2
        {
            return AgentTier::Reasoning;
        }

        let developer_task_markers = [
            "fetch",
            "scrape",
            "crawl",
            "extract",
            "read file",
            "inspect file",
            "compare data",
            "dataset",
            "csv",
            "json",
            "table",
            "diff",
            "search",
            "lookup",
            "抓取",
            "爬取",
            "提取",
            "读取",
            "查看文件",
            "文件",
            "对比",
            "比较数据",
            "数据",
            "表格",
            "搜索",
            "查询资料",
            "整理",
        ];
        if has_tools
            || developer_task_markers
                .iter()
                .any(|marker| lower.contains(marker))
        {
            return AgentTier::Medium;
        }

        let trimmed_len = text.trim().chars().count();
        if !has_tools
            && trimmed_len <= 40
            && matches!(
                task_type,
                TaskType::Chat | TaskType::Translation | TaskType::Summarization
            )
        {
            return AgentTier::Simple;
        }

        AgentTier::Medium
    }

    fn all_text(request: &ModelRequest) -> String {
        request
            .messages
            .iter()
            .map(Self::message_text)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn calculate_complexity(request: &ModelRequest, task_type: &TaskType) -> f64 {
        let mut score = 0.0;

        // 消息数量
        let msg_count = request.messages.len() as f64;
        score += (msg_count / 20.0).min(0.3);

        // 是否有工具调用
        if request.tools.as_ref().is_some_and(|t| !t.is_empty()) {
            score += 0.2;
        }

        // 是否有图片
        if Self::has_images(request) {
            score += 0.15;
        }

        // token 数量
        let tokens = Self::estimate_tokens(request);
        score += (tokens as f64 / 100_000.0).min(0.15);

        // 任务类型
        match task_type {
            TaskType::Code | TaskType::Analysis => score += 0.15,
            TaskType::Creative => score += 0.1,
            _ => score += 0.05,
        }

        // max_tokens 大说明期望长输出
        if let Some(max_tok) = request.max_tokens {
            score += (max_tok as f64 / 16_384.0).min(0.05);
        }

        score.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(messages: Vec<Message>) -> ModelRequest {
        ModelRequest {
            model: "claude-sonnet-4-6".into(),
            messages,
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn detects_code_task() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("请帮我写一个 Rust function 来实现快速排序".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.task_type, TaskType::Code);
    }

    #[test]
    fn detects_chat_task() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("你好，今天天气怎么样？".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.task_type, TaskType::Chat);
    }

    #[test]
    fn detects_tools() {
        let mut req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("查询天气".into()),
        }]);
        req.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "get_weather".into(),
                description: None,
                parameters: None,
            },
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert!(features.has_tools);
        assert!(features.complexity_score > 0.0);
    }

    #[test]
    fn estimate_tokens_is_reasonable() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("Hello, world!".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert!(features.estimated_tokens > 0);
        assert!(features.estimated_tokens < 1000);
    }

    #[test]
    fn classifies_simple_agent_tier() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("你好".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Simple);
    }

    #[test]
    fn classifies_medium_agent_tier_for_single_code_task() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("写一个 TypeScript debounce 函数".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Medium);
    }

    #[test]
    fn classifies_developer_retrieval_tasks_as_medium_not_simple() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("抓取这个接口的数据，并对比两个 JSON 文件的差异".into()),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Medium);
    }

    #[test]
    fn classifies_tool_using_tasks_as_medium_not_simple() {
        let mut req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text("读取文件".into()),
        }]);
        req.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "read_file".into(),
                description: None,
                parameters: None,
            },
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Medium);
    }

    #[test]
    fn classifies_reasoning_agent_tier_for_deep_code_work() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text(
                "审查这个项目的架构，分析安全风险，然后给出重构方案".into(),
            ),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Reasoning);
    }

    #[test]
    fn classifies_project_level_implementation_as_reasoning() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text(
                "帮我从零实现一个 Rust 服务，包含错误处理、配置加载、API 路由和测试。".into(),
            ),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Reasoning);
    }

    #[test]
    fn classifies_complex_agent_tier_for_parallel_coordination() {
        let req = make_request(vec![Message {
            role: "user".into(),
            content: MessageContent::Text(
                "同时让前端、后端、安全三个子 agent 并行完成这个系统".into(),
            ),
        }]);
        let features = RequestAnalyzer::analyze(&req);
        assert_eq!(features.agent_tier, AgentTier::Complex);
    }

    #[test]
    fn agent_harness_context_does_not_raise_latest_user_intent_tier() {
        let mut req = make_request(vec![
            Message {
                role: "system".into(),
                content: MessageContent::Text(
                    "You are a coding agent. Review architecture, security, tests, tools, and \
                     orchestrate parallel sub-agents.\n"
                        .repeat(2_000),
                ),
            },
            Message {
                role: "user".into(),
                content: MessageContent::Text("继续".into()),
            },
        ]);
        req.tools = Some(vec![Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "exec_command".into(),
                description: Some("Run shell commands and inspect the project".into()),
                parameters: Some(serde_json::json!({"type": "object"})),
            },
        }]);

        let features = RequestAnalyzer::analyze(&req);

        assert_eq!(features.agent_tier, AgentTier::Simple);
        assert!(features.has_tools);
        assert!(features.estimated_tokens > 32_000);
        assert!(features.is_long_context);
    }

    #[test]
    fn synthetic_environment_message_does_not_replace_real_user_intent() {
        let req = make_request(vec![
            Message {
                role: "user".into(),
                content: MessageContent::Text("你好".into()),
            },
            Message {
                role: "user".into(),
                content: MessageContent::Text(
                    "<environment_context>parallel agents architecture security audit</environment_context>"
                        .into(),
                ),
            },
        ]);

        let features = RequestAnalyzer::analyze(&req);

        assert_eq!(features.task_type, TaskType::Chat);
        assert_eq!(features.agent_tier, AgentTier::Simple);
    }

    #[test]
    fn tool_result_message_does_not_replace_real_user_intent() {
        let req = make_request(vec![
            Message {
                role: "user".into(),
                content: MessageContent::Text("你好".into()),
            },
            Message {
                role: "assistant".into(),
                content: MessageContent::Text("[Tool call exec_command id=1]".into()),
            },
            Message {
                role: "user".into(),
                content: MessageContent::Text(
                    "[Tool result id=1]\narchitecture security audit failed tests".into(),
                ),
            },
        ]);

        let features = RequestAnalyzer::analyze(&req);

        assert_eq!(features.task_type, TaskType::Chat);
        assert_eq!(features.agent_tier, AgentTier::Simple);
    }
}
