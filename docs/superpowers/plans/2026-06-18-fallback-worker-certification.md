# Fallback Worker Certification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Dispatcher `v0.3.0` as the fallback worker certification layer for Dispatcher 2.0 Codex handoff continuation.

**Architecture:** Add model-level handoff certification metadata to provider/model capabilities, keep the eval fixture harness local and deterministic, and apply certification filtering only to tagged handoff continuations. Ordinary `provider-auto` routing remains unchanged; certified workers are preferred only when the request is continuing a `dispatcher_handoff.v1` package.

**Tech Stack:** Rust workspace (`dispatcher-engine`, `dispatcher-providers`, `dispatcher-server`), SQLite telemetry through `rusqlite`, TypeScript/React dashboard, existing i18n JSON, Markdown docs.

---

## Scope Lock

This is not Dispatcher 3.0 and not a generic model switcher.

Allowed:

- Model-level handoff certification schema.
- Built-in fallback worker eval fixture definitions.
- Handoff-only eligibility filtering for requests tagged with `handoff_package_id`.
- Provider/API/telemetry/dashboard visibility for certification state and chosen worker evidence.
- Local docs, project manual updates, version prep for `v0.3.0`.

Not allowed:

- Claude Code-first behavior.
- Default silent model switching.
- Hosted Responses tool emulation for third-party providers.
- Claims that domestic or fallback models are equivalent to native Codex.
- Publishing to `origin`.
- Pushing or tagging without explicit user confirmation.

## File Structure

- `crates/dispatcher-engine/src/handoff_certification.rs`: new certification domain types, built-in fixture loading, required-label logic, and capability filtering helpers.
- `crates/dispatcher-engine/src/handoff_eval_fixtures.json`: deterministic fixture catalog for text-only, code-patch, tool-capable, and long-context handoff worker certification.
- `crates/dispatcher-engine/src/lib.rs`: export the new certification module and helpers.
- `crates/dispatcher-engine/src/types.rs`: add `handoff_certification` to `ModelInfo` and `ProviderScore`.
- `crates/dispatcher-engine/src/scorer.rs` and `selector.rs` tests: preserve score construction and selected candidate metadata.
- `crates/dispatcher-providers/src/metadata.rs`: parse nested `handoff_certification` metadata from bundled or operator-supplied TOML.
- `crates/dispatcher-providers/provider-models.toml`: give every bundled candidate an explicit profile; only demo gets local text-only demo certification, production providers stay explicit until evidence is supplied.
- `crates/dispatcher-server/src/routes/responses.rs`: filter `provider-auto` capabilities only when `handoff_package_id` is present, record certification labels with continuation telemetry, and return a clear error when no certified worker is eligible.
- `crates/dispatcher-server/src/routes/providers.rs`: expose model-level `handoff_certification` through `/v1/providers`.
- `crates/dispatcher-server/src/telemetry.rs`: persist and expose continuation certification labels and eligibility reason.
- `web/src/types.ts`: add certification response types.
- `web/src/components/SimpleDashboard.tsx` and `web/src/components/QuickTestPanel.tsx`: show certified worker count and selected continuation labels without claiming equivalence.
- `web/src/i18n/locales/en.json` and `web/src/i18n/locales/zh.json`: add short UI labels.
- `README.md`: document certified fallback workers under Codex routing modes.
- `docs/dispatcher-2.0/PROJECT_MANUAL.md`: record v0.3.0 in-progress state and next verification.
- `docs/releases/v0.3.0-fallback-worker-certification.md`: draft local release notes.
- `Cargo.toml`, `package.json`, `web/package.json`, `Cargo.lock`: align local version to `0.3.0` only after functional verification passes.

## Task 1: Certification Schema And Fixture Harness

**Files:**
- Create: `crates/dispatcher-engine/src/handoff_certification.rs`
- Create: `crates/dispatcher-engine/src/handoff_eval_fixtures.json`
- Modify: `crates/dispatcher-engine/src/lib.rs`
- Modify: `crates/dispatcher-engine/src/types.rs`
- Modify: `crates/dispatcher-engine/src/scorer.rs`
- Modify: `crates/dispatcher-engine/src/selector.rs`

- [ ] **Step 1: Write failing certification schema tests**

Add tests in `crates/dispatcher-engine/src/handoff_certification.rs`:

```rust
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
        assert_eq!(certification.labels, vec![HandoffCertificationLabel::NotCertified]);
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
        let certified = capability(model(vec![HandoffCertificationLabel::HandoffTextOnly], 8192));

        let (eligible, excluded) = filter_handoff_eligible_capabilities(&[certified], &features);

        assert_eq!(eligible.len(), 1);
        assert!(excluded.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p dispatcher-engine handoff_certification --lib
```

Expected: FAIL because `handoff_certification` module and types do not exist.

- [ ] **Step 3: Implement certification types and helpers**

Implement:

```rust
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
            let reason = handoff_model_rejection_reason(model, required);
            if let Some(reason) = reason {
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
        return Some(format!("handoff worker missing {required:?} certification").to_lowercase());
    }
    None
}
```

- [ ] **Step 4: Add fixture JSON**

Create `crates/dispatcher-engine/src/handoff_eval_fixtures.json`:

```json
[
  {
    "id": "text_only_status_recap",
    "required_label": "handoff_text_only",
    "prompt": "Given a dispatcher_handoff.v1 package, summarize the objective, current status, hazards, and safest next step without inventing hidden context.",
    "success_criteria": [
      "mentions the objective",
      "mentions degraded fallback limits",
      "does not claim native Codex equivalence"
    ]
  },
  {
    "id": "code_patch_continuation",
    "required_label": "handoff_code_patch",
    "prompt": "Given a dispatcher_handoff.v1 package for a Rust/TypeScript change, propose a minimal patch plan and verification commands.",
    "success_criteria": [
      "keeps changes scoped to listed files",
      "includes verification commands",
      "does not fabricate tool results"
    ]
  },
  {
    "id": "tool_call_argument_integrity",
    "required_label": "handoff_tool_capable",
    "prompt": "Given a handoff package and a JSON-schema function, produce a valid tool call with exact arguments required by the package.",
    "success_criteria": [
      "emits valid JSON arguments",
      "does not call unavailable hosted tools",
      "preserves package id"
    ]
  },
  {
    "id": "long_context_handoff_reconstruction",
    "required_label": "handoff_long_context",
    "prompt": "Given a long dispatcher_handoff.v1 package and surrounding notes, identify contradictions and produce a bounded continuation summary.",
    "success_criteria": [
      "uses the handoff package as source of truth",
      "flags contradictions",
      "does not rely on hidden reasoning state"
    ]
  }
]
```

- [ ] **Step 5: Wire type fields**

Add `pub mod handoff_certification;` and public re-exports in `crates/dispatcher-engine/src/lib.rs`.

Add to `ModelInfo`:

```rust
#[serde(default)]
pub handoff_certification: HandoffCertification,
```

Add to `ProviderScore`:

```rust
pub handoff_certification: HandoffCertification,
```

Copy `model.handoff_certification.clone()` into `ProviderScorer` score creation and update test helper constructors.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p dispatcher-engine handoff_certification --lib
cargo test -p dispatcher-engine scorer --lib
cargo test -p dispatcher-engine selector --lib
```

Expected: PASS.

## Task 2: Provider Metadata Certification Profiles

**Files:**
- Modify: `crates/dispatcher-providers/src/metadata.rs`
- Modify: `crates/dispatcher-providers/provider-models.toml`
- Test: `crates/dispatcher-providers/src/metadata.rs`

- [ ] **Step 1: Write failing metadata test**

Add a metadata test proving nested certification metadata is parsed:

```rust
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
handoff_certification = {
  labels = ["handoff_text_only", "handoff_code_patch"],
  eval_set = "dispatcher-handoff-v0.3.0-fixtures",
  evaluated_at = "2026-06-18",
  notes = "fixture-backed test profile"
}
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
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p dispatcher-providers handoff_certification --lib
```

Expected: FAIL because metadata parsing does not know `handoff_certification`.

- [ ] **Step 3: Implement metadata parsing**

Import certification types:

```rust
use dispatcher_engine::types::{HandoffCertification, ModelInfo, ProviderCapability};
```

Add to `ModelMetadata`:

```rust
handoff_certification: Option<HandoffCertification>,
```

Validate:

```rust
fn validate_handoff_certification(value: Option<&HandoffCertification>) -> anyhow::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    anyhow::ensure!(!value.labels.is_empty(), "handoff_certification.labels must not be empty");
    validate_non_empty(value.eval_set.as_deref(), "handoff_certification.eval_set")?;
    validate_date(value.evaluated_at.as_deref())?;
    Ok(())
}
```

Apply:

```rust
if let Some(handoff_certification) = &override_model.handoff_certification {
    model.handoff_certification = handoff_certification.clone();
}
```

Initialize `model_from_metadata` with `handoff_certification.unwrap_or_default()`.

- [ ] **Step 4: Update bundled metadata profiles**

Add explicit `handoff_certification` to bundled models. Use `not_certified` for production providers unless there is current fixture evidence. Mark demo only as local text-only demo:

```toml
handoff_certification = { labels = ["handoff_text_only"], eval_set = "dispatcher-demo-local", evaluated_at = "2026-06-18", notes = "Local demo text fixture only; not a production fallback worker." }
```

For production provider models:

```toml
handoff_certification = { labels = ["not_certified"], notes = "Requires operator eval evidence before use as a handoff worker." }
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p dispatcher-providers metadata --lib
cargo test -p dispatcher-providers registry --lib
```

Expected: PASS.

## Task 3: Handoff-Only Eligibility Filtering

**Files:**
- Modify: `crates/dispatcher-server/src/routes/responses.rs`
- Test: `crates/dispatcher-server/src/routes/responses.rs`

- [ ] **Step 1: Write failing route tests**

Add tests around existing provider-auto handoff continuation tests:

```rust
#[tokio::test]
async fn provider_auto_handoff_requires_certified_worker() {
    let state = test_state_with_capabilities(vec![test_capability_with_certification(
        "uncertified",
        "fast-model",
        crate::handoff_certification::HandoffCertification::default(),
    )])
    .await;
    let mut request = route_request("Summarize this handoff package.");
    request
        .extra
        .insert("handoff_package_id".into(), serde_json::json!("pkg_uncertified"));

    let response = provider_responses(state.clone(), request.clone(), responses_to_model_request(&request)).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let stats = state.telemetry.get_stats().await.unwrap();
    assert_eq!(stats["latest_handoff_continuation"]["provider_id"], "unavailable");
    assert_eq!(
        stats["latest_handoff_continuation"]["eligibility_reason"],
        "no certified fallback worker supports this handoff continuation"
    );
}

#[tokio::test]
async fn provider_auto_without_handoff_package_keeps_generic_routing() {
    let state = test_state_with_capabilities(vec![test_capability_with_certification(
        "uncertified",
        "fast-model",
        crate::handoff_certification::HandoffCertification::default(),
    )])
    .await;
    let request = route_request("hello generic provider-auto");

    let response = provider_responses(state, request.clone(), responses_to_model_request(&request)).await;

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p dispatcher-server responses --lib
```

Expected: FAIL because handoff-only certification filtering is not implemented.

- [ ] **Step 3: Filter capabilities only for tagged handoff continuations**

In `provider_responses`, after building `capabilities`, apply:

```rust
let features = dispatcher_engine::RequestAnalyzer::analyze(&model_request);
let (capabilities, handoff_exclusions, handoff_eligibility_reason) =
    if handoff_package_id.is_some() {
        let (eligible, excluded) = dispatcher_engine::filter_handoff_eligible_capabilities(
            &capabilities,
            &features,
        );
        (
            eligible,
            excluded,
            Some("selected from certified fallback workers".to_string()),
        )
    } else {
        (capabilities, Vec::new(), None)
    };
```

When no decision exists for a tagged handoff, record:

```rust
"No certified fallback worker supports this handoff continuation"
```

When a decision exists, append `handoff_exclusions` to `decision.excluded_candidates`, and pass `handoff_eligibility_reason` into continuation recording.

- [ ] **Step 4: Keep fallback attempts within certified candidate set**

Ensure `provider_attempts` receives a decision whose `candidates` came from filtered capabilities. Do not call the registry capability list again when building fallback attempts.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p dispatcher-server responses --lib
```

Expected: PASS.

## Task 4: Telemetry And API Visibility

**Files:**
- Modify: `crates/dispatcher-server/src/telemetry.rs`
- Modify: `crates/dispatcher-server/src/routes/providers.rs`
- Modify: `crates/dispatcher-server/src/routes/responses.rs`
- Test: `crates/dispatcher-server/src/telemetry.rs`
- Test: `crates/dispatcher-server/src/routes/providers.rs`

- [ ] **Step 1: Write failing telemetry/API tests**

Add telemetry assertion:

```rust
#[tokio::test]
async fn telemetry_stats_include_handoff_continuation_certification() {
    let db_path = temp_db_path("dispatcher-handoff-certification-telemetry");
    let store = TelemetryStore::new(db_path.to_string_lossy().as_ref()).await.unwrap();

    store
        .record_handoff_continuation(&HandoffContinuationRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            package_id: "pkg_123".into(),
            provider_id: "demo".into(),
            model_id: "demo-echo".into(),
            success: true,
            status_code: Some(200),
            latency_ms: 25,
            response_text: Some("continued".into()),
            error_message: None,
            source: "user_click".into(),
            status: "succeeded".into(),
            certification_labels: vec!["handoff_text_only".into()],
            eligibility_reason: Some("selected from certified fallback workers".into()),
        })
        .await
        .unwrap();

    let stats = store.get_stats().await.unwrap();
    let continuation = &stats["latest_handoff_continuation"];

    assert_eq!(continuation["certification_labels"][0], "handoff_text_only");
    assert_eq!(
        continuation["eligibility_reason"],
        "selected from certified fallback workers"
    );

    drop(store);
    std::fs::remove_file(db_path).unwrap();
}
```

Add provider route assertion that `/v1/providers` model JSON includes `handoff_certification.labels`.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p dispatcher-server telemetry_stats_include_handoff_continuation_certification --lib
cargo test -p dispatcher-server providers_response_includes_model_metadata_source_and_effective_capabilities --lib
```

Expected: FAIL because fields do not exist in telemetry/API.

- [ ] **Step 3: Persist continuation certification fields**

Add columns:

```sql
certification_labels TEXT NOT NULL DEFAULT '[]',
eligibility_reason TEXT
```

Add migration in `ensure_handoff_continuation_metadata_columns`.

Add fields to `HandoffContinuationRecord`:

```rust
pub certification_labels: Vec<String>,
pub eligibility_reason: Option<String>,
```

Serialize labels with `serde_json::to_string(&record.certification_labels)?` and parse them in `get_stats`.

- [ ] **Step 4: Expose provider model certification**

In `/v1/providers` model JSON add:

```rust
"handoff_certification": m.handoff_certification,
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p dispatcher-server telemetry --lib
cargo test -p dispatcher-server providers --lib
```

Expected: PASS.

## Task 5: Dashboard Visibility

**Files:**
- Modify: `web/src/types.ts`
- Modify: `web/src/components/SimpleDashboard.tsx`
- Modify: `web/src/components/QuickTestPanel.tsx`
- Modify: `web/src/i18n/locales/en.json`
- Modify: `web/src/i18n/locales/zh.json`

- [ ] **Step 1: Add TypeScript types**

Add:

```ts
export type HandoffCertificationLabel =
  | "handoff_text_only"
  | "handoff_code_patch"
  | "handoff_tool_capable"
  | "handoff_long_context"
  | "not_certified";

export interface HandoffCertification {
  labels: HandoffCertificationLabel[];
  eval_set?: string | null;
  evaluated_at?: string | null;
  notes?: string | null;
}
```

Add `handoff_certification: HandoffCertification` to `ModelInfo`, and add `certification_labels: HandoffCertificationLabel[]` plus `eligibility_reason: string | null` to `HandoffContinuationTelemetry`.

- [ ] **Step 2: Show certified worker count in simple dashboard**

Compute:

```ts
const certifiedWorkerCount = providers.flatMap((provider) =>
  provider.models.filter((model) =>
    model.handoff_certification.labels.some((label) => label !== "not_certified"),
  ),
).length;
```

Add a `SimpleSignal` labeled `dashboard.certifiedWorkers` with detail `dashboard.certifiedWorkersDetail`.

- [ ] **Step 3: Show selected continuation certification**

In `SimplePersistedContinuation` and `PersistedContinuationResult`, render labels when present:

```tsx
{continuation.certification_labels.map((label) => (
  <code key={label}>{formatHandoffValue(label)}</code>
))}
```

Render `eligibility_reason` in the existing result paragraph or meta row. Keep wording degraded and bounded.

- [ ] **Step 4: Add i18n strings**

English:

```json
"certifiedWorkers": "Certified handoff workers",
"certifiedWorkersDetail": "Model-level handoff evidence",
"workerCertification": "Worker certification",
"eligibilityReason": "Eligibility"
```

Chinese:

```json
"certifiedWorkers": "已认证交接 worker",
"certifiedWorkersDetail": "模型级交接证据",
"workerCertification": "Worker 认证",
"eligibilityReason": "准入原因"
```

- [ ] **Step 5: Run frontend verification**

Run:

```bash
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
```

Expected: PASS.

## Task 6: Docs, Version Prep, And Final Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/dispatcher-2.0/PROJECT_MANUAL.md`
- Create: `docs/releases/v0.3.0-fallback-worker-certification.md`
- Modify: `Cargo.toml`
- Modify: `package.json`
- Modify: `web/package.json`
- Modify: `Cargo.lock`

- [ ] **Step 1: Update docs**

README must say:

- `v0.3.0` adds fallback worker certification for Codex handoff continuation.
- Certification is model-level, not provider-level.
- Uncertified models are not eligible for tagged handoff continuation.
- This does not make fallback models equivalent to Codex.
- `provider-auto` generic routing remains available outside tagged handoff continuation.

Project manual must record:

- v0.3.0 scope.
- Current local implementation status.
- No push to `origin`.
- Next verification commands.

Release draft must include included/not included/verification sections.

- [ ] **Step 2: Version prep**

After the targeted Rust/frontend tests pass, update:

```toml
[workspace.package]
version = "0.3.0"
```

Update root and web package versions to `0.3.0`, then run:

```bash
cargo check --workspace
```

This updates `Cargo.lock` package versions.

- [ ] **Step 3: Run full verification matrix**

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
git diff --check
```

Expected: PASS.

- [ ] **Step 4: Local service smoke check**

Only if no service already listens on 8787:

```bash
lsof -nP -iTCP:8787 -sTCP:LISTEN
cargo run -- serve --web-dir ./web/dist
curl --noproxy '*' -i http://127.0.0.1:8787/v1/health
curl --noproxy '*' -s http://127.0.0.1:8787/v1/providers | rg "handoff_certification|demo-echo"
```

Expected:

- `/v1/health` returns HTTP 200 with `version` set to `0.3.0`.
- `/v1/providers` includes `handoff_certification` for models.
- Stop any temporary server started for this check.

## Self-Review

- Spec coverage: covers all requested focus areas: model-level schema, eval fixtures, handoff eligibility filtering, telemetry/API/dashboard visibility, docs and verification.
- Placeholder scan: no `TBD`, `TODO`, or unspecified implementation step remains.
- Type consistency: `HandoffCertification`, `HandoffCertificationLabel`, `certification_labels`, and `eligibility_reason` are consistently named across Rust, API JSON, and TypeScript.
- Scope check: generic provider routing remains outside the certification filter; only tagged handoff continuation uses it.
