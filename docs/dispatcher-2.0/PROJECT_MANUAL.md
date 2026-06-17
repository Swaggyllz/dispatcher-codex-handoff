# Dispatcher 2.0 Project Manual

Last updated: 2026-06-17

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

## Release Strategy

Dispatcher 2.0 should be published as a new GitHub project, not pushed over the existing Dispatcher 1.0 repository.

Recommended release shape:

- Keep the original GitHub 1.0 repository unchanged.
- Create a new GitHub repository for 2.0 when it is ready to publish.
- Recommended repository names:
  - `dispatcher-codex-handoff`
  - `codex-handoff-router`
  - `dispatcher-2`
- Do not replace the existing `origin` remote unless the user explicitly asks.
- Prefer adding a second remote for the 2.0 repository:

```bash
git remote add v2 git@github.com:<user-or-org>/dispatcher-codex-handoff.git
git push -u v2 main
```

Before publishing 2.0, run a release readiness pass:

- Confirm README and docs describe Dispatcher 2.0 / Codex Handoff Router, not the old 1.0 positioning.
- Confirm environment variable examples are accurate.
- Confirm screenshots or dashboard descriptions match the implemented 2.0 UI.
- Confirm `docs/dispatcher-2.0/PROJECT_MANUAL.md` is current.
- Confirm no local secrets, private URLs, or accidental test artifacts are included.
- Prepare initial release notes for the new repository.

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
- Task 3 committed:
  - `5b722b8 feat: create codex emergency handoffs`
  - File:
    - `crates/dispatcher-server/src/routes/responses.rs`
  - Creates emergency handoff packages on native Codex `429` / quota-like responses and preserves upstream response status/body.
- Review fix committed:
  - `a4fe5ee fix: keep codex handoff user request scoped`
  - File:
    - `crates/dispatcher-server/src/routes/responses.rs`
  - Ensures `latest_user_request` is derived from user messages only.
- Task 4 committed:
  - `3a0d621 feat: show codex handoff status`
  - Files:
    - `web/src/types.ts`
    - `web/src/App.tsx`
    - `web/src/components/QuickTestPanel.tsx`
    - `web/src/i18n/locales/en.json`
    - `web/src/i18n/locales/zh.json`
  - Shows the latest emergency handoff in dashboard telemetry.
- Review fix committed:
  - `1ca4a8a fix: narrow codex handoff telemetry types`
  - File:
    - `web/src/types.ts`
  - Narrows frontend handoff telemetry union types.
- Task 5 documentation complete:
  - Documentation now states the first slice is emergency handoff only.
  - Task 5 verification has been run.
- Format follow-up complete:
  - `cargo fmt --all` was run after Task 5 surfaced formatting diffs.
  - Files formatted:
    - `crates/dispatcher-server/src/handoff.rs`
    - `crates/dispatcher-server/src/routes/responses.rs`
- Continuation prompt copy action complete:
  - Dashboard handoff cards now include a user-approved copy action for `continuation_prompt`.
  - This does not execute fallback models or switch providers automatically.
  - Verification passed: `pnpm --dir web format:check`, `pnpm --dir web typecheck`, and `pnpm --dir web build`.
- Rate-limit header headroom parsing complete:
  - `QuotaSignal::from_response` now reads reliable `x-ratelimit-limit-*` and `x-ratelimit-remaining-*` header pairs.
  - It records the minimum normalized headroom when both `limit` and `remaining` are present.
  - Native Codex responses with header-derived headroom record a quota event even when they are not emergency handoffs.
  - This does not trigger planned 10% handoff and does not switch providers automatically.
- Dashboard quota signal display complete:
  - `c3c0e1f feat: show codex quota headroom`
  - Quick Test now shows the latest observed Codex quota signal from `/v1/telemetry.latest_quota_event`.
  - It displays normalized headroom only when reliable upstream rate-limit header pairs were observed.
  - This is observational telemetry only; it does not trigger planned 10% handoff or automatic fallback execution.
- User-approved fallback continuation complete:
  - `82498fb feat: continue codex handoffs via provider auto`
  - `a5752b8 feat: show fallback continuation route`
  - Dashboard handoff cards now include a user-clicked continuation action.
  - The action sends `continuation_prompt` to `POST /v1/responses` with `X-Dispatcher-Mode: provider-auto`.
  - The continuation result displays the observed fallback provider/model from dispatcher response headers.
  - It is still explicit user approval, not automatic fallback execution.
- Primary-route recovery review complete:
  - Provider-auto handoff continuations are tagged with `handoff_package_id`.
  - Dispatcher persists the latest fallback continuation and exposes it as `/v1/telemetry.latest_handoff_continuation`.
  - Dashboard shows the saved fallback continuation only when it matches the current handoff package.
  - Dashboard can copy a primary-route review prompt. This does not automatically switch back to Codex.
  - Current MVP records non-streaming provider-auto continuation results; streaming continuation persistence remains a future hardening task.
- App-like simple dashboard complete:
  - Dashboard now opens in a simple Codex handoff overview by default and keeps the previous detailed dashboard behind Professional mode.
  - The simple view shows latest handoff state, explicit fallback continuation, copy handoff prompt, latest model, quota signal, and observed provider health.
  - Provider health wording now distinguishes "awaiting health samples" from local service health so unknown providers are not presented as healthy.
  - Web metadata now uses Dispatcher 2.0 / Codex Handoff Router and includes the app icon favicon in the Vite build output.
- Release version alignment complete:
  - Workspace, root package, web package, Cargo.lock, CLI help, and CLI crate metadata now use `0.2.0` / Codex Handoff Router release positioning.

Current milestone status:

- First slice complete: native Codex quota/rate-limit emergency handoff package persistence and dashboard visibility.
- Second slice complete: user-approved provider-auto continuation from the emergency handoff card.
- Release preparation started:
  - `docs/releases/v0.2.0-codex-handoff.md` drafts initial Dispatcher 2.0 release notes and PR summary.
  - README handoff documentation now describes emergency package visibility and explicit user-approved continuation.
  - New GitHub repository created: `https://github.com/Swaggyllz/dispatcher-codex-handoff`.
  - Local `v2` remote added for the new repository. Existing `origin` still points to `https://github.com/Swaggyllz/dispatcher.git`.
  - README and package metadata now lead with Dispatcher 2.0 / Codex Handoff Router and point quick start instructions at the new repository.
  - `main` was pushed to `v2/main`; the local `main` branch now tracks `v2/main`.
  - New repository check passed: public repo, default branch `main`, README renders with Dispatcher 2.0 heading.
  - `v0.2.0` tag was created and pushed to `v2`.
- Verification status: full release readiness verification passed again after app-like dashboard, release metadata, and version alignment updates on 2026-06-17.
- Still pending for future phases: planned handoff from reliable quota snapshots, automatic background fallback execution, streaming continuation persistence, and richer primary-route reclaim workflows.

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
- `HandoffContinuationRecord`
- `record_quota_event`
- `record_handoff_package`
- `record_handoff_continuation`
- telemetry JSON fields:
  - `latest_quota_event`
  - `latest_handoff`
  - `latest_handoff_continuation`

## Next Steps

Immediate:

- Do not push to `origin main`.
- Optional: create a GitHub Release for the already-pushed `v0.2.0` tag on the new `v2` repository.
- If any code or release-configuration changes are made after `v0.2.0`, rerun the full verification matrix before publishing another tag.

Future planned phases:

- Planned handoff from reliable quota snapshots. Header-derived normalized headroom is now recorded, but no planned handoff trigger has been implemented.
- Automatic background fallback execution.
- Streaming continuation persistence.
- Richer primary-route reclaim workflows.

Final verification reference:

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

Latest final verification result after app-like dashboard and v0.2.0 release alignment updates on 2026-06-17:

- `./scripts/check-open-source-readiness.sh`: passed.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
- `cargo check --workspace`: passed.
- `pnpm --dir web format:check`: passed.
- `pnpm --dir web typecheck`: passed.
- `pnpm --dir web build`: passed.

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
