use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaSignal {
    pub is_emergency: bool,
    pub status_code: Option<u16>,
    pub retry_after_secs: Option<u64>,
    pub normalized_headroom: Option<f64>,
    pub source: String,
}

impl QuotaSignal {
    pub fn from_response(status: StatusCode, headers: &HeaderMap) -> Self {
        let retry_after_secs = headers
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        Self {
            is_emergency: status == StatusCode::TOO_MANY_REQUESTS || retry_after_secs.is_some(),
            status_code: Some(status.as_u16()),
            retry_after_secs,
            normalized_headroom: None,
            source: if status == StatusCode::TOO_MANY_REQUESTS {
                "http_429".into()
            } else if retry_after_secs.is_some() {
                "retry_after".into()
            } else {
                "http_status".into()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffPackage {
    pub schema_version: String,
    pub package_id: String,
    pub created_at: DateTime<Utc>,
    pub trigger: String,
    pub confidence: String,
    pub objective: String,
    pub latest_user_request: String,
    pub current_status: String,
    pub completion_criteria: Vec<String>,
    pub workspace: HandoffWorkspace,
    pub execution_state: HandoffExecutionState,
    pub technical_context: HandoffTechnicalContext,
    pub routing_context: HandoffRoutingContext,
    pub continuation_prompt: String,
    pub hazards: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffWorkspace {
    pub cwd: String,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub dirty_state: String,
    pub touched_files: Vec<String>,
    pub relevant_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffExecutionState {
    pub mode: String,
    pub last_successful_step: Option<String>,
    pub next_recommended_step: String,
    pub blocked_on: Option<String>,
    pub commands_run: Vec<String>,
    pub verification_run: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffTechnicalContext {
    pub key_findings: Vec<String>,
    pub decisions_made: Vec<String>,
    pub assumptions: Vec<String>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffRoutingContext {
    pub agent_tier: String,
    pub requested_model: String,
    pub selected_model: String,
    pub reasoning_effort: String,
    pub speed: String,
    pub dispatcher_mode: String,
}

pub struct EmergencyHandoffInput {
    pub requested_model: String,
    pub selected_model: String,
    pub reasoning_effort: String,
    pub speed: String,
    pub agent_tier: String,
    pub dispatcher_mode: String,
    pub latest_user_request: String,
    pub cwd: String,
    pub error_message: String,
    pub signal: QuotaSignal,
}

impl EmergencyHandoffInput {
    pub fn build(self) -> HandoffPackage {
        let next_step = "Inspect relevant files and confirm current task state before editing.";
        HandoffPackage {
            schema_version: "dispatcher_handoff.v1".into(),
            package_id: format!("handoff_{}", uuid::Uuid::new_v4().simple()),
            created_at: Utc::now(),
            trigger: if self.signal.status_code == Some(429) {
                "rate_limit_429".into()
            } else {
                "quota_warning".into()
            },
            confidence: "emergency_reconstruction".into(),
            objective: "Continue the interrupted Codex task from observable state.".into(),
            latest_user_request: self.latest_user_request.clone(),
            current_status: "blocked".into(),
            completion_criteria: vec![
                "Audit the current workspace state before editing.".into(),
                "Continue only within the latest user request and handoff constraints.".into(),
            ],
            workspace: HandoffWorkspace {
                cwd: self.cwd,
                repo_name: None,
                branch: None,
                dirty_state: "unknown".into(),
                touched_files: Vec::new(),
                relevant_files: Vec::new(),
            },
            execution_state: HandoffExecutionState {
                mode: "research_only".into(),
                last_successful_step: None,
                next_recommended_step: next_step.into(),
                blocked_on: Some(self.error_message),
                commands_run: Vec::new(),
                verification_run: Vec::new(),
            },
            technical_context: HandoffTechnicalContext {
                key_findings: Vec::new(),
                decisions_made: Vec::new(),
                assumptions: vec![
                    "Previous model was interrupted before producing a handoff summary.".into(),
                    "State was reconstructed from observable routing and request data only.".into(),
                ],
                constraints: vec![
                    "Do not perform broad refactors.".into(),
                    "Do not run destructive git commands.".into(),
                    "Stop and report if workspace state contradicts this handoff.".into(),
                ],
            },
            routing_context: HandoffRoutingContext {
                agent_tier: self.agent_tier,
                requested_model: self.requested_model,
                selected_model: self.selected_model,
                reasoning_effort: self.reasoning_effort,
                speed: self.speed,
                dispatcher_mode: self.dispatcher_mode,
            },
            continuation_prompt: format!(
                "You are continuing an interrupted Dispatcher Codex task.\n\nLatest user request:\n{}\n\nCurrent status:\nThe native Codex route hit quota pressure. This package is an emergency reconstruction from observable state, not a full model-authored summary.\n\nDo next:\n1. {}\n2. Re-read relevant files before editing.\n3. If the state is unclear, report what is missing instead of guessing.\n\nDo not:\n- Perform broad refactors.\n- Run destructive git commands.\n- Assume hidden context that is not written here.",
                self.latest_user_request, next_step
            ),
            hazards: vec![
                "Emergency handoff may be missing the interrupted model's intent.".into(),
                "Fallback model must audit state before implementation.".into(),
            ],
            open_questions: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_failure_detects_429_and_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "300".parse().unwrap());

        let signal = QuotaSignal::from_response(StatusCode::TOO_MANY_REQUESTS, &headers);

        assert!(signal.is_emergency);
        assert_eq!(signal.status_code, Some(429));
        assert_eq!(signal.retry_after_secs, Some(300));
        assert_eq!(signal.source, "http_429");
    }

    #[test]
    fn quota_failure_detects_retry_after_without_429() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "120".parse().unwrap());

        let signal = QuotaSignal::from_response(StatusCode::SERVICE_UNAVAILABLE, &headers);

        assert!(signal.is_emergency);
        assert_eq!(signal.status_code, Some(503));
        assert_eq!(signal.retry_after_secs, Some(120));
        assert_eq!(signal.normalized_headroom, None);
        assert_eq!(signal.source, "retry_after");
    }

    #[test]
    fn emergency_package_is_reconstruction_with_guardrails() {
        let signal = QuotaSignal {
            is_emergency: true,
            status_code: Some(429),
            retry_after_secs: Some(300),
            normalized_headroom: None,
            source: "http_429".into(),
        };
        let package = EmergencyHandoffInput {
            requested_model: "gpt-5.5".into(),
            selected_model: "gpt-5.5".into(),
            reasoning_effort: "xhigh".into(),
            speed: "priority".into(),
            agent_tier: "complex".into(),
            dispatcher_mode: "auto".into(),
            latest_user_request: "Implement the quota fallback.".into(),
            cwd: "/workspace/dispatcher".into(),
            error_message: "Codex upstream returned HTTP 429 Too Many Requests".into(),
            signal,
        }
        .build();

        assert_eq!(package.schema_version, "dispatcher_handoff.v1");
        assert_eq!(package.trigger, "rate_limit_429");
        assert_eq!(package.confidence, "emergency_reconstruction");
        assert_eq!(package.execution_state.mode, "research_only");
        assert!(
            package
                .continuation_prompt
                .contains("Inspect relevant files and confirm current task state before editing.")
        );
        assert!(
            package
                .technical_context
                .assumptions
                .iter()
                .any(|item| item.contains("observable"))
        );
    }
}
