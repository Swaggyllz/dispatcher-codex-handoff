# Dispatcher 2.0 Project Manual

Last updated: 2026-06-16

## Purpose

This is the continuation manual for Dispatcher 2.0. If a new Codex window needs to continue the project, read this file first.

Dispatcher 2.0 is defined as:

```text
Codex Handoff Router
```

The product goal is to help Codex users continue work after the native Codex route hits quota pressure or rate limits. The first milestone is not seamless migration. It is an honest emergency handoff:

```text
Codex native 429 / quota error
-> create dispatcher_handoff.v1 emergency package
-> persist quota event + handoff package
-> show latest handoff in telemetry/dashboard
```

## Non-Negotiable Scope Boundaries

Do not drift from these boundaries unless the user explicitly changes direction.

- Codex first. Do not implement Claude Code behavior in this milestone.
- Emergency handoff first. Do not implement planned 10% handoff in this milestone.
- No exact quota promise. Do not claim official "10% remaining" unless reliable upstream headers exist and are normalized.
- No automatic fallback execution in this milestone.
- No hosted Responses tool emulation for third-party providers.
- No hidden reasoning or context migration.
- Fallback models are degraded execution workers, not Codex-native replacements.

## Current Implementation Status

Completed:

- Research package committed:
  - `28a0c90 docs: outline dispatcher 2.0 codex handoff`
  - Files: `docs/dispatcher-2.0/00-pm-brief.md` through `06-cross-check-report.md`
- Implementation plan committed:
  - `045b566 docs: plan codex emergency handoff`
  - File: `docs/superpowers/plans/2026-06-16-codex-emergency-handoff.md`
- Task 1 committed:
  - `5be7941 feat: add codex handoff schema`
  - Files:
    - `crates/dispatcher-server/src/handoff.rs`
    - `crates/dispatcher-server/src/lib.rs`
  - Adds `dispatcher_handoff.v1`, `QuotaSignal`, and emergency package builder.
- Task 2 committed:
  - `a25d7c4 feat: persist quota and handoff telemetry`
  - File:
    - `crates/dispatcher-server/src/telemetry.rs`
  - Adds `quota_events`, `handoff_packages`, `record_quota_event`, `record_handoff_package`, `latest_quota_event`, and `latest_handoff`.

In progress:

- Task 3: create emergency handoff on native Codex 429.

Pending:

- Task 4: dashboard handoff display.
- Task 5: docs and verification.

## Source Of Truth Documents

Read these in order:

1. `docs/dispatcher-2.0/PROJECT_MANUAL.md`
2. `docs/dispatcher-2.0/00-pm-brief.md`
3. `docs/dispatcher-2.0/06-cross-check-report.md`
4. `docs/superpowers/plans/2026-06-16-codex-emergency-handoff.md`

Use the implementation plan as the executable checklist. Use this manual as the project state tracker.

## Architecture Summary

Existing Codex paths:

- Native Codex route:
  - `POST /v1/responses`
  - `X-Dispatcher-Mode: auto`
  - implemented in `crates/dispatcher-server/src/routes/responses.rs`
  - forwards to Codex/OpenAI upstream and preserves native Responses behavior.
- Provider fallback route:
  - `POST /v1/responses`
  - `X-Dispatcher-Mode: provider-auto`
  - converts Responses input to internal `ModelRequest`
  - routes through configured providers.

New 2.0 first-slice data path:

```text
native Codex response status/headers
-> QuotaSignal::from_response
-> quota_events row
-> EmergencyHandoffInput::build
-> handoff_packages row
-> /v1/telemetry latest_handoff
-> dashboard display
```

## Current Data Structures

`crates/dispatcher-server/src/handoff.rs` contains:

- `QuotaSignal`
- `HandoffPackage`
- `HandoffWorkspace`
- `HandoffExecutionState`
- `HandoffTechnicalContext`
- `HandoffRoutingContext`
- `EmergencyHandoffInput`

`crates/dispatcher-server/src/telemetry.rs` contains:

- `QuotaEventRecord`
- `record_quota_event`
- `record_handoff_package`
- telemetry JSON fields:
  - `latest_quota_event`
  - `latest_handoff`

## Next Task: Task 3

Task 3 title:

```text
Create Emergency Handoff On Native Codex 429
```

Allowed files:

- `crates/dispatcher-server/src/routes/responses.rs`

What to implement:

- Add `latest_user_text(&ResponsesRequest) -> String`.
- Pass the original `ResponsesRequest` into `forward_codex_response`.
- After native Codex upstream response status is known, call `QuotaSignal::from_response(status, response.headers())`.
- If `is_emergency`:
  - record `QuotaEventRecord`
  - build `EmergencyHandoffInput`
  - persist `HandoffPackage`
  - add diagnostic response headers:
    - `x-dispatcher-handoff: emergency`
    - `x-dispatcher-handoff-confidence: emergency_reconstruction`
- Keep upstream status and body unchanged.

What not to implement:

- Do not retry through `provider-auto`.
- Do not add planned handoff logic.
- Do not add dashboard UI in Task 3.
- Do not touch provider metadata.

Suggested tests:

- `latest_user_text_extracts_last_message_text`
- `codex_quota_signal_detects_429`
- Existing `responses` tests must continue to pass.

Command:

```bash
cargo test -p dispatcher-server responses --lib
```

## Remaining Tasks After Task 3

Task 4:

- Add frontend types for `latest_quota_event` and `latest_handoff`.
- Pass `latest_handoff` to `QuickTestPanel`.
- Show latest handoff status near the latest Codex route.
- Add English and Chinese i18n strings.
- Run:

```bash
pnpm --dir web typecheck
pnpm --dir web build
```

Task 5:

- Update `docs/dispatcher-2.0/05-mvp-scope-and-plan.md`.
- Update `README.zh-CN.md` with the Codex handoff experiment note.
- Run backend and frontend verification.

Final verification:

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

## Subagent Management Rules

Use subagents, but keep them boxed in:

- One implementation task per worker.
- Do not dispatch parallel implementation workers that write overlapping files.
- Give each worker exact allowed files.
- After each worker returns, inspect the diff before continuing.
- If a worker adds Claude Code, planned 10%, automatic fallback execution, broad refactors, or unrelated UI, reject and correct it.

## Product Acceptance Criteria For This Milestone

The milestone is done only when:

- A simulated native Codex `429` can create a persisted emergency handoff package.
- The package has `schema_version = dispatcher_handoff.v1`.
- The package has `confidence = emergency_reconstruction`.
- `/v1/telemetry` returns the latest handoff.
- Dashboard shows the latest handoff clearly as emergency/degraded.
- Existing Codex native routing behavior remains unchanged except for telemetry and diagnostic headers.
- Existing provider-auto behavior remains unchanged.

## Current User Intent

The user explicitly said:

```text
最后指挥子agent干出来的活不要偏离我们最开始设定的就好。
必须要有一个项目说明书，如果我要重开窗口继续做这个项目，可以直接读取进度，且之后怎么操作，进行到哪一步了，之后的几步是什么样子。
```

So this manual must stay current whenever a task completes.

