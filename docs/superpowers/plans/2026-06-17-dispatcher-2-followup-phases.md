# Dispatcher 2.0 Follow-up Phases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the four post-`v0.2.0` Dispatcher 2.0 phases while keeping the product Codex-first and stable.

**Architecture:** Add quota snapshot and continuation metadata to the existing telemetry store, keep native Codex transport unchanged, and reuse provider-auto for fallback continuation. UI changes stay inside the existing dashboard surfaces and remain explicit/user-approved unless an operator opts into background auto continuation.

**Tech Stack:** Rust, Axum, rusqlite, React, TypeScript, Vite.

---

## File Structure

- `crates/dispatcher-server/src/handoff.rs`: quota snapshot parsing, planned handoff trigger helpers, planned package builder.
- `crates/dispatcher-server/src/telemetry.rs`: quota snapshots and continuation source/status telemetry.
- `crates/dispatcher-server/src/routes/responses.rs`: native Codex planned trigger, background continuation scheduling, streaming continuation persistence.
- `crates/dispatcher-server/src/routes/responses_compat.rs`: expose completed streaming text for persistence.
- `web/src/types.ts`: telemetry type additions.
- `web/src/lib/api/dashboard.ts`: optional primary-review request helper.
- `web/src/components/SimpleDashboard.tsx`: default UI reclaim/background state.
- `web/src/components/QuickTestPanel.tsx`: professional UI labels/status.
- `web/src/i18n/locales/en.json` and `web/src/i18n/locales/zh.json`: user-facing copy.
- `docs/dispatcher-2.0/PROJECT_MANUAL.md`: continuation state and next steps.

## Tasks

### Task 1: Quota Snapshots And Planned Handoff

- [x] Add failing tests for reliable quota snapshot parsing and threshold gating.
- [x] Implement snapshot parsing and planned trigger helpers in `handoff.rs`.
- [x] Add telemetry storage and latest snapshot JSON in `telemetry.rs`.
- [x] Persist snapshots and planned packages from native Codex responses.
- [x] Verify with focused Rust tests.

### Task 2: Continuation Source And Background Auto Continuation

- [x] Add failing telemetry tests for continuation `source` and `status`.
- [x] Add migration-safe columns with defaults for existing databases.
- [x] Add `DISPATCHER_HANDOFF_AUTO_CONTINUE` helper, default off.
- [x] Schedule one background provider-auto continuation after handoff package persistence when enabled.
- [x] Verify auto-disabled and auto-enabled route behavior.

### Task 3: Streaming Continuation Persistence

- [x] Add failing stream tests for tagged streaming continuation persistence.
- [x] Expose final stream text from `ResponsesStreamState`.
- [x] Pass `handoff_package_id` through streaming provider-auto.
- [x] Persist terminal stream success/failure only.
- [x] Verify existing SSE behavior remains unchanged.

### Task 4: Primary Reclaim UI

- [x] Add TypeScript types for snapshot/source/status telemetry.
- [x] Add simple dashboard state for saved fallback continuation and primary review.
- [x] Add explicit copy-review and review-with-Codex actions.
- [x] Update simple dashboard labels for planned/background states.
- [x] Verify web format, typecheck, and build.

### Task 5: Docs, Release Readiness, And Full Verification

- [x] Update `PROJECT_MANUAL.md` with completed follow-up phases.
- [x] Update README, env example, and follow-up docs; no version change has been made.
- [x] Run the full verification matrix.
- [ ] Commit and push only to `v2` if the user asks for publication.
