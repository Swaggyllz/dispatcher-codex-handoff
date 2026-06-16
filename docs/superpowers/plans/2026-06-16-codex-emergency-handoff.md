# Codex Emergency Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Dispatcher 2.0 slice: native Codex quota/rate-limit failures create a persisted `dispatcher_handoff.v1` emergency handoff package and expose it in telemetry/dashboard.

**Architecture:** Keep Codex native transport unchanged. Add quota/handoff telemetry alongside existing `codex_routes`, generate handoff only on observable emergency conditions, and surface the latest package through the existing `/v1/telemetry` polling path.

**Tech Stack:** Rust 1.95+, Axum, reqwest, rusqlite, serde, React, TypeScript, TanStack Query, i18next.

---

## File Structure

Create:

- `crates/dispatcher-server/src/handoff.rs`: handoff schema, emergency package builder, quota header parser helpers.

Modify:

- `crates/dispatcher-server/src/lib.rs`: export `handoff` module.
- `crates/dispatcher-server/src/telemetry.rs`: add SQLite tables, record/get methods, telemetry JSON fields, tests.
- `crates/dispatcher-server/src/routes/responses.rs`: capture native Codex upstream status/headers and create emergency handoff on quota-like failures.
- `web/src/types.ts`: add handoff telemetry types.
- `web/src/App.tsx`: pass latest handoff to dashboard panel.
- `web/src/components/QuickTestPanel.tsx`: show latest handoff near latest Codex route.
- `web/src/i18n/locales/en.json`: add handoff strings.
- `web/src/i18n/locales/zh.json`: add handoff strings.
- `docs/dispatcher-2.0/05-mvp-scope-and-plan.md`: mark Phase 2/3 implementation boundary as started.

Test:

- `crates/dispatcher-server/src/handoff.rs` unit tests.
- `crates/dispatcher-server/src/telemetry.rs` unit tests.
- `crates/dispatcher-server/src/routes/responses.rs` route/helper tests.
- `pnpm --dir web typecheck`.

## Task 1: Handoff Schema Module

**Files:**

- Create: `crates/dispatcher-server/src/handoff.rs`
- Modify: `crates/dispatcher-server/src/lib.rs`
- Test: `crates/dispatcher-server/src/handoff.rs`

- [ ] **Step 1: Add failing schema tests**

Create `crates/dispatcher-server/src/handoff.rs` with tests first:

```rust
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        assert!(package
            .continuation_prompt
            .contains("Inspect relevant files and confirm current task state before editing."));
        assert!(package
            .technical_context
            .assumptions
            .iter()
            .any(|item| item.contains("observable")));
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p dispatcher-server handoff --lib
```

Expected: fail because `QuotaSignal`, `EmergencyHandoffInput`, and package structs are not implemented.

- [ ] **Step 3: Implement handoff structs and helpers**

Add the implementation above the tests in `crates/dispatcher-server/src/handoff.rs`:

```rust
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
                "You are continuing an interrupted Dispatcher Codex task.\n\nLatest user request:\n{}\n\nCurrent status:\nThe native Codex route hit quota pressure. This package is an emergency reconstruction, not a full model-authored summary.\n\nDo next:\n1. {}\n2. Re-read relevant files before editing.\n3. If the state is unclear, report what is missing instead of guessing.\n\nDo not:\n- Perform broad refactors.\n- Run destructive git commands.\n- Assume hidden context that is not written here.",
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
```

- [ ] **Step 4: Export the module**

Modify `crates/dispatcher-server/src/lib.rs`:

```rust
pub mod handoff;
```

Place it next to the existing module declarations near the top of the file.

- [ ] **Step 5: Verify tests pass**

Run:

```bash
cargo test -p dispatcher-server handoff --lib
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/dispatcher-server/src/handoff.rs crates/dispatcher-server/src/lib.rs
git commit -m "feat: add codex handoff schema"
```

## Task 2: Persist Quota Events And Handoff Packages

**Files:**

- Modify: `crates/dispatcher-server/src/telemetry.rs`
- Test: `crates/dispatcher-server/src/telemetry.rs`

- [ ] **Step 1: Add failing telemetry tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `telemetry.rs`:

```rust
#[tokio::test]
async fn telemetry_stats_include_latest_handoff_package() {
    let db_path = temp_db_path("dispatcher-handoff-telemetry");
    let store = TelemetryStore::new(db_path.to_string_lossy().as_ref())
        .await
        .unwrap();
    let package = crate::handoff::EmergencyHandoffInput {
        requested_model: "gpt-5.5".into(),
        selected_model: "gpt-5.5".into(),
        reasoning_effort: "xhigh".into(),
        speed: "priority".into(),
        agent_tier: "complex".into(),
        dispatcher_mode: "auto".into(),
        latest_user_request: "Finish the implementation".into(),
        cwd: "/workspace/dispatcher".into(),
        error_message: "Codex upstream returned HTTP 429 Too Many Requests".into(),
        signal: crate::handoff::QuotaSignal {
            is_emergency: true,
            status_code: Some(429),
            retry_after_secs: Some(120),
            normalized_headroom: None,
            source: "http_429".into(),
        },
    }
    .build();

    store.record_handoff_package(&package).await.unwrap();

    let stats = store.get_stats().await.unwrap();
    let handoff = &stats["latest_handoff"];
    assert_eq!(handoff["schema_version"], "dispatcher_handoff.v1");
    assert_eq!(handoff["trigger"], "rate_limit_429");
    assert_eq!(handoff["confidence"], "emergency_reconstruction");
    assert_eq!(handoff["latest_user_request"], "Finish the implementation");
}

#[tokio::test]
async fn telemetry_stats_include_latest_quota_event() {
    let db_path = temp_db_path("dispatcher-quota-telemetry");
    let store = TelemetryStore::new(db_path.to_string_lossy().as_ref())
        .await
        .unwrap();
    let event = QuotaEventRecord {
        id: "quota_test".into(),
        timestamp: chrono::Utc::now(),
        provider_id: "codex".into(),
        model_id: "gpt-5.5".into(),
        status_code: Some(429),
        retry_after_secs: Some(120),
        normalized_headroom: None,
        source: "http_429".into(),
    };

    store.record_quota_event(&event).await.unwrap();

    let stats = store.get_stats().await.unwrap();
    assert_eq!(stats["latest_quota_event"]["provider_id"], "codex");
    assert_eq!(stats["latest_quota_event"]["status_code"], 429);
    assert_eq!(stats["latest_quota_event"]["retry_after_secs"], 120);
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p dispatcher-server telemetry_stats_include_latest --lib
```

Expected: fail because the record types, tables, and methods do not exist.

- [ ] **Step 3: Add telemetry record type**

Near `CodexTelemetryRecord`, add:

```rust
#[derive(Debug, Clone)]
pub struct QuotaEventRecord {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub provider_id: String,
    pub model_id: String,
    pub status_code: Option<u16>,
    pub retry_after_secs: Option<u64>,
    pub normalized_headroom: Option<f64>,
    pub source: String,
}
```

- [ ] **Step 4: Add SQLite tables and indexes**

Extend the `execute_batch` string in `TelemetryStore::new`:

```sql
CREATE TABLE IF NOT EXISTS quota_events (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    status_code INTEGER,
    retry_after_secs INTEGER,
    normalized_headroom REAL,
    source TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS handoff_packages (
    package_id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    trigger TEXT NOT NULL,
    confidence TEXT NOT NULL,
    latest_user_request TEXT NOT NULL,
    package_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quota_events_timestamp ON quota_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_handoff_packages_created_at ON handoff_packages(created_at);
```

- [ ] **Step 5: Add record methods**

Add methods on `TelemetryStore`:

```rust
pub async fn record_quota_event(&self, record: &QuotaEventRecord) -> anyhow::Result<()> {
    let db = self.db.lock().await;
    db.execute(
        "INSERT INTO quota_events (
            id, timestamp, provider_id, model_id, status_code, retry_after_secs,
            normalized_headroom, source
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.id,
            record.timestamp.to_rfc3339(),
            record.provider_id,
            record.model_id,
            record.status_code,
            record.retry_after_secs,
            record.normalized_headroom,
            record.source,
        ],
    )?;
    Ok(())
}

pub async fn record_handoff_package(
    &self,
    package: &crate::handoff::HandoffPackage,
) -> anyhow::Result<()> {
    let db = self.db.lock().await;
    db.execute(
        "INSERT OR REPLACE INTO handoff_packages (
            package_id, created_at, schema_version, trigger, confidence,
            latest_user_request, package_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            package.package_id,
            package.created_at.to_rfc3339(),
            package.schema_version,
            package.trigger,
            package.confidence,
            package.latest_user_request,
            serde_json::to_string(package)?,
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 6: Add latest telemetry JSON fields**

Inside `get_stats_at`, query latest records before the final `serde_json::json!`:

```rust
let latest_quota_event = db
    .query_row(
        "SELECT timestamp, provider_id, model_id, status_code, retry_after_secs,
                normalized_headroom, source
         FROM quota_events
         ORDER BY timestamp DESC, rowid DESC
         LIMIT 1",
        [],
        |row| {
            Ok(serde_json::json!({
                "timestamp": row.get::<_, String>(0)?,
                "provider_id": row.get::<_, String>(1)?,
                "model_id": row.get::<_, String>(2)?,
                "status_code": row.get::<_, Option<i64>>(3)?,
                "retry_after_secs": row.get::<_, Option<i64>>(4)?,
                "normalized_headroom": row.get::<_, Option<f64>>(5)?,
                "source": row.get::<_, String>(6)?,
            }))
        },
    )
    .optional()?;

let latest_handoff = db
    .query_row(
        "SELECT package_json
         FROM handoff_packages
         ORDER BY created_at DESC, rowid DESC
         LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok());
```

Then add fields to the returned JSON:

```rust
"latest_quota_event": latest_quota_event,
"latest_handoff": latest_handoff,
```

- [ ] **Step 7: Run telemetry tests**

Run:

```bash
cargo test -p dispatcher-server telemetry --lib
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add crates/dispatcher-server/src/telemetry.rs
git commit -m "feat: persist quota and handoff telemetry"
```

## Task 3: Create Emergency Handoff On Native Codex 429

**Files:**

- Modify: `crates/dispatcher-server/src/routes/responses.rs`
- Test: `crates/dispatcher-server/src/routes/responses.rs`

- [ ] **Step 1: Add helper tests**

Add tests near the existing Codex routing tests:

```rust
#[test]
fn latest_user_text_extracts_last_message_text() {
    let request = ResponsesRequest {
        model: "gpt-5.5".into(),
        instructions: None,
        input: vec![
            ResponseInputItem {
                item_type: "message".into(),
                role: Some("user".into()),
                content: vec![ResponseContentPart {
                    content_type: "input_text".into(),
                    text: Some("first".into()),
                    image_url: None,
                }],
                name: None,
                arguments: None,
                call_id: None,
                output: None,
            },
            ResponseInputItem {
                item_type: "message".into(),
                role: Some("user".into()),
                content: vec![ResponseContentPart {
                    content_type: "input_text".into(),
                    text: Some("second".into()),
                    image_url: None,
                }],
                name: None,
                arguments: None,
                call_id: None,
                output: None,
            },
        ],
        tools: vec![],
        stream: false,
        max_output_tokens: None,
        temperature: None,
        reasoning: None,
        service_tier: None,
        extra: Default::default(),
    };

    assert_eq!(latest_user_text(&request), "second");
}

#[test]
fn codex_quota_signal_detects_429() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", "60".parse().unwrap());

    let signal = crate::handoff::QuotaSignal::from_response(StatusCode::TOO_MANY_REQUESTS, &headers);

    assert!(signal.is_emergency);
    assert_eq!(signal.retry_after_secs, Some(60));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p dispatcher-server latest_user_text_extracts_last_message_text --lib
cargo test -p dispatcher-server codex_quota_signal_detects_429 --lib
```

Expected: first test fails because `latest_user_text` does not exist.

- [ ] **Step 3: Implement latest user extraction**

Add helper in `responses.rs`:

```rust
fn latest_user_text(request: &ResponsesRequest) -> String {
    request
        .input
        .iter()
        .rev()
        .find(|item| {
            item.item_type == "message"
                && matches!(item.role.as_deref().unwrap_or("user"), "user" | "developer")
        })
        .and_then(|item| {
            item.content
                .iter()
                .filter_map(|part| part.text.as_deref())
                .find(|text| !text.trim().is_empty())
        })
        .unwrap_or("")
        .to_string()
}
```

- [ ] **Step 4: Pass request into forward function**

Change `forward_codex_response` signature:

```rust
async fn forward_codex_response(
    state: &Arc<AppState>,
    original_request: &ResponsesRequest,
    requested_model: &str,
    upstream_body: serde_json::Value,
    route: &CodexRoute,
    stream: bool,
    auth: Option<CodexAuth>,
) -> axum::response::Response
```

Update the call site to pass `&request`.

- [ ] **Step 5: Record quota event and handoff after upstream response status**

After `let status = response.status();`, before `record_codex_outcome`, add:

```rust
let quota_signal = crate::handoff::QuotaSignal::from_response(status, response.headers());
if quota_signal.is_emergency {
    let event = crate::telemetry::QuotaEventRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        provider_id: "codex".into(),
        model_id: effective_route.model.clone(),
        status_code: quota_signal.status_code,
        retry_after_secs: quota_signal.retry_after_secs,
        normalized_headroom: quota_signal.normalized_headroom,
        source: quota_signal.source.clone(),
    };
    if let Err(error) = state.telemetry.record_quota_event(&event).await {
        tracing::warn!("Failed to record Codex quota event: {error}");
    }

    let package = crate::handoff::EmergencyHandoffInput {
        requested_model: requested_model.into(),
        selected_model: effective_route.model.clone(),
        reasoning_effort: effective_route.reasoning_effort.clone(),
        speed: codex_speed_label(effective_route.speed).into(),
        agent_tier: format!("{:?}", effective_route.agent_tier).to_lowercase(),
        dispatcher_mode: "auto".into(),
        latest_user_request: latest_user_text(original_request),
        cwd: std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        error_message: format!("Codex upstream returned HTTP {status}"),
        signal: quota_signal,
    }
    .build();
    if let Err(error) = state.telemetry.record_handoff_package(&package).await {
        tracing::warn!("Failed to record Codex handoff package: {error}");
    }
}
```

- [ ] **Step 6: Add diagnostic response headers**

When `quota_signal.is_emergency`, add these headers to the response builder:

```rust
builder = builder.header("x-dispatcher-handoff", "emergency");
builder = builder.header("x-dispatcher-handoff-confidence", "emergency_reconstruction");
```

Keep the upstream status/body unchanged.

- [ ] **Step 7: Run response tests**

Run:

```bash
cargo test -p dispatcher-server responses --lib
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add crates/dispatcher-server/src/routes/responses.rs
git commit -m "feat: create codex emergency handoffs"
```

## Task 4: Dashboard Telemetry Types And Handoff Display

**Files:**

- Modify: `web/src/types.ts`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/QuickTestPanel.tsx`
- Modify: `web/src/i18n/locales/en.json`
- Modify: `web/src/i18n/locales/zh.json`

- [ ] **Step 1: Extend TypeScript telemetry types**

In `web/src/types.ts`, add:

```ts
export interface QuotaEventTelemetry {
  timestamp: string;
  provider_id: string;
  model_id: string;
  status_code: number | null;
  retry_after_secs: number | null;
  normalized_headroom: number | null;
  source: string;
}

export interface HandoffPackageTelemetry {
  schema_version: "dispatcher_handoff.v1";
  package_id: string;
  created_at: string;
  trigger: "planned" | "quota_warning" | "rate_limit_429" | "manual";
  confidence: "strong_summary" | "emergency_reconstruction";
  objective: string;
  latest_user_request: string;
  current_status: string;
  continuation_prompt: string;
  hazards: string[];
  open_questions: string[];
  execution_state: {
    mode: "plan_only" | "research_only" | "edit_allowed" | "verify_only";
    next_recommended_step: string;
    blocked_on: string | null;
    commands_run: string[];
    verification_run: string[];
  };
  routing_context: {
    agent_tier: AgentTier;
    requested_model: string;
    selected_model: string;
    reasoning_effort: "low" | "medium" | "high" | "xhigh";
    speed: "standard" | "priority";
    dispatcher_mode: string;
  };
}
```

Extend `TelemetryStats`:

```ts
latest_quota_event: QuotaEventTelemetry | null;
latest_handoff: HandoffPackageTelemetry | null;
```

- [ ] **Step 2: Pass handoff to QuickTestPanel**

In `web/src/App.tsx`, update:

```tsx
<QuickTestPanel
  latestCodexRoute={telemetry?.latest_codex_route}
  latestHandoff={telemetry?.latest_handoff}
/>
```

- [ ] **Step 3: Update QuickTestPanel props**

In `web/src/components/QuickTestPanel.tsx`, import `HandoffPackageTelemetry` and update props:

```tsx
export function QuickTestPanel({
  latestCodexRoute,
  latestHandoff,
}: {
  latestCodexRoute?: CodexRouteTelemetry | null;
  latestHandoff?: HandoffPackageTelemetry | null;
}) {
```

After `CodexResult`, render:

```tsx
{latestHandoff && <HandoffResult handoff={latestHandoff} t={t} />}
```

- [ ] **Step 4: Add HandoffResult component**

Add below `CodexResult`:

```tsx
function HandoffResult({
  handoff,
  t,
}: {
  handoff: HandoffPackageTelemetry;
  t: (key: string) => string;
}) {
  return (
    <div className="handoff-result">
      <div className="route-result-title">
        <div>
          <span>{t("dashboard.latestHandoff")}</span>
          <strong>{t(`dashboard.handoffConfidence.${handoff.confidence}`)}</strong>
        </div>
        <span className="native-route-badge">{t("dashboard.handoffMode")}</span>
      </div>
      <div className="route-properties">
        <RouteProperty
          label={t("dashboard.handoffTrigger")}
          value={t(`dashboard.handoffTriggerValue.${handoff.trigger}`)}
        />
        <RouteProperty
          label={t("dashboard.executionMode")}
          value={t(`dashboard.executionModeValue.${handoff.execution_state.mode}`)}
        />
        <RouteProperty
          label={t("common.model")}
          value={handoff.routing_context.selected_model}
        />
      </div>
      <div className="selection-basis codex-selection-basis">
        <span>{t("dashboard.nextRecommendedStep")}</span>
        <strong>{handoff.execution_state.next_recommended_step}</strong>
      </div>
      <p className="codex-route-error">{handoff.latest_user_request}</p>
    </div>
  );
}
```

- [ ] **Step 5: Add i18n strings**

Add to both locale files under `dashboard`.

English:

```json
"latestHandoff": "Latest handoff",
"handoffMode": "Handoff",
"handoffTrigger": "Trigger",
"executionMode": "Execution mode",
"nextRecommendedStep": "Next recommended step",
"handoffConfidence": {
  "strong_summary": "Strong summary",
  "emergency_reconstruction": "Emergency reconstruction"
},
"handoffTriggerValue": {
  "planned": "Planned",
  "quota_warning": "Quota warning",
  "rate_limit_429": "Rate limit",
  "manual": "Manual"
},
"executionModeValue": {
  "plan_only": "Plan only",
  "research_only": "Research only",
  "edit_allowed": "Edit allowed",
  "verify_only": "Verify only"
}
```

Chinese:

```json
"latestHandoff": "最新交接包",
"handoffMode": "交接",
"handoffTrigger": "触发原因",
"executionMode": "执行模式",
"nextRecommendedStep": "建议下一步",
"handoffConfidence": {
  "strong_summary": "强模型摘要",
  "emergency_reconstruction": "应急重建"
},
"handoffTriggerValue": {
  "planned": "提前交接",
  "quota_warning": "额度预警",
  "rate_limit_429": "限流",
  "manual": "手动"
},
"executionModeValue": {
  "plan_only": "仅规划",
  "research_only": "仅研究",
  "edit_allowed": "允许编辑",
  "verify_only": "仅验证"
}
```

- [ ] **Step 6: Add minimal CSS if needed**

If layout needs spacing, add to `web/src/index.css`:

```css
.handoff-result {
  margin-top: 12px;
  border-top: 1px solid var(--border-subtle);
  padding-top: 12px;
}
```

Use existing classes first; only add CSS if the component visually collapses.

- [ ] **Step 7: Run frontend checks**

Run:

```bash
pnpm --dir web typecheck
pnpm --dir web build
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add web/src/types.ts web/src/App.tsx web/src/components/QuickTestPanel.tsx web/src/i18n/locales/en.json web/src/i18n/locales/zh.json web/src/index.css
git commit -m "feat: show codex handoff status"
```

## Task 5: Documentation And Verification

**Files:**

- Modify: `docs/dispatcher-2.0/05-mvp-scope-and-plan.md`
- Modify: `README.zh-CN.md`

- [ ] **Step 1: Update MVP doc**

In `docs/dispatcher-2.0/05-mvp-scope-and-plan.md`, add a short implementation status section:

```markdown
## Implementation Status

Phase 2/3 first slice:

- Structured quota event storage is implemented for native Codex emergency signals.
- Emergency `dispatcher_handoff.v1` package persistence is implemented.
- Dashboard telemetry shows the latest emergency handoff package.

Still pending:

- Planned 10% handoff from reliable quota snapshots.
- Automatic fallback execution through `provider-auto`.
- Primary-route recovery review workflow.
```

- [ ] **Step 2: Add README alpha note**

In `README.zh-CN.md`, under Codex routing modes, add:

```markdown
### Codex 交接模式实验

Dispatcher 2.0 的第一阶段会在 Codex 原生路由遇到 429 或明确额度压力时记录
`dispatcher_handoff.v1` 应急交接包。该交接包会出现在控制台 telemetry 中，帮助用户
把当前任务交给备用模型继续。第一阶段只做可观测交接，不自动承诺 10% 精确余额、
不模拟托管工具，也不自动切换到第三方模型。
```

- [ ] **Step 3: Run full backend checks for touched crates**

Run:

```bash
cargo fmt --all --check
cargo test -p dispatcher-server
cargo check --workspace
```

Expected: pass.

- [ ] **Step 4: Run frontend checks**

Run:

```bash
pnpm --dir web typecheck
pnpm --dir web build
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add docs/dispatcher-2.0/05-mvp-scope-and-plan.md README.zh-CN.md
git commit -m "docs: document codex handoff experiment"
```

## Final Verification

Run:

```bash
./scripts/check-open-source-readiness.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
```

Expected: all pass.

## Implementation Notes

- Keep emergency handoff user-approved. This plan does not auto-run fallback models.
- Keep quota state separate from circuit breaker state.
- Do not add Claude Code behavior in this slice.
- Do not forward Codex bearer tokens to third-party providers.
- Do not claim exact `10%` remaining unless reliable headers are captured and normalized.
- Do not store secret headers. Only persist numeric quota fields, status code, source, and reset/retry timing.
