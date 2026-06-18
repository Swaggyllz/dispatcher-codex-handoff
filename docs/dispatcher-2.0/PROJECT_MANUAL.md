# Dispatcher 2.0 Project Manual

Last updated: 2026-06-18

## Purpose

This is the continuation manual for Dispatcher 2.0. If a new Codex window needs to continue the project, read this file first.

Dispatcher 2.0 is defined as:

```text
Codex Handoff Router
```

The product goal is to help Codex users continue work after the native Codex route hits quota pressure or rate limits. The first milestone was not seamless migration. It was an honest emergency handoff:

```text
Codex native 429 / quota error
-> create dispatcher_handoff.v1 emergency package
-> persist quota event + handoff package
-> show latest handoff in telemetry/dashboard
```

The post-`v0.2.0` follow-up phase extends that foundation with planned handoff
from reliable quota snapshots, explicitly configured background fallback
continuation, streaming continuation persistence, and primary-route review.

## Non-Negotiable Scope Boundaries

Do not drift from these boundaries unless the user explicitly changes direction.

- Codex first. Do not implement Claude Code behavior in this milestone.
- Emergency handoff was first for `v0.2.0`. Planned handoff is now allowed only
  from reliable upstream rate-limit header pairs.
- No exact quota promise. Do not claim official "10% remaining" unless reliable upstream headers exist and are normalized.
- No automatic fallback execution unless `DISPATCHER_HANDOFF_AUTO_CONTINUE=1`
  is explicitly configured by the operator.
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
- Post-`v0.2.0` follow-up implementation complete in the current workspace:
  - Reliable rate-limit header pairs now persist quota snapshots and can create
    planned handoff packages when normalized headroom is at or below
    `DISPATCHER_PLANNED_HANDOFF_THRESHOLD` (default `0.10`).
  - Emergency handoff behavior remains unchanged for 429 / `retry-after`.
  - Background fallback continuation is off by default and only runs when
    `DISPATCHER_HANDOFF_AUTO_CONTINUE=1`.
  - Tagged streaming provider-auto continuations now persist terminal success
    text or terminal failure state.
  - The simple dashboard exposes saved fallback continuation state, source /
    status, copy-review-prompt, and explicit "Review with Codex".
- Release version alignment for the follow-up patch complete:
  - Workspace, root package, web package, and Cargo.lock now use `0.2.1`.
  - Release notes added:
    `docs/releases/v0.2.1-codex-handoff-followup.md`.

Current milestone status:

- First slice complete: native Codex quota/rate-limit emergency handoff package persistence and dashboard visibility.
- Second slice complete: user-approved provider-auto continuation from the emergency handoff card.
- Follow-up phases complete and published as `v0.2.1`: planned reliable
  quota-snapshot handoff, opt-in background continuation, streaming
  continuation persistence, and richer primary-route reclaim UI.
- Local `v0.3.0` fallback worker certification implementation is complete in
  the current workspace:
  - Model-level `handoff_certification` profiles have been added to provider
    model metadata and `/v1/providers`.
  - Built-in handoff eval fixtures define text-only, code-patch, tool-capable,
    and long-context certification labels.
  - Tagged `provider-auto` handoff continuations now filter candidates by
    certification; ordinary untagged `provider-auto` routing remains unchanged.
  - Handoff continuation telemetry records selected certification labels and
    eligibility reason.
  - Dashboard surfaces certified worker count and saved continuation
    certification state.
  - Version metadata has been prepared as `0.3.0`.
  - This is local preparation only. It has not been pushed, tagged, or released.
- Release publication:
  - `docs/releases/v0.2.0-codex-handoff.md` drafts initial Dispatcher 2.0 release notes and PR summary.
  - `docs/releases/v0.2.1-codex-handoff-followup.md` records the follow-up
    patch release notes.
  - README handoff documentation now describes emergency package visibility and explicit user-approved continuation.
  - New GitHub repository created: `https://github.com/Swaggyllz/dispatcher-codex-handoff`.
  - Local `v2` remote added for the new repository. Existing `origin` still points to `https://github.com/Swaggyllz/dispatcher.git`.
  - README and package metadata now lead with Dispatcher 2.0 / Codex Handoff Router and point quick start instructions at the new repository.
  - `main` was pushed to `v2/main`; the local `main` branch now tracks `v2/main`.
  - New repository check passed: public repo, default branch `main`, README renders with Dispatcher 2.0 heading.
  - `v0.2.0` tag was created and pushed to `v2`.
  - GitHub Release `v0.2.0` was created on `Swaggyllz/dispatcher-codex-handoff`.
  - Follow-up commit `74bf2c3 feat: complete dispatcher 2.0 handoff follow-up`
    was pushed to `v2/main`.
  - Release preparation commit
    `2fa536f chore: prepare v0.2.1 release` was pushed to `v2/main`.
  - `v0.2.1` tag was created from the release preparation commit and pushed
    to `v2`.
  - GitHub Release `v0.2.1` was created on
    `Swaggyllz/dispatcher-codex-handoff`:
    `https://github.com/Swaggyllz/dispatcher-codex-handoff/releases/tag/v0.2.1`.
- Verification status: full release readiness verification passed again for
  `v0.2.1` on 2026-06-18 before publication.

Current workspace checkpoint on 2026-06-18:

- Branch is `main` tracking `v2/main`.
- `origin` still points to the Dispatcher 1.0 repository:
  `https://github.com/Swaggyllz/dispatcher.git`.
- `v2` points to the Dispatcher 2.0 repository:
  `https://github.com/Swaggyllz/dispatcher-codex-handoff.git`.
- Do not push to `origin`. Use only `v2` as the Dispatcher 2.0 publication
  target unless the user explicitly changes the release strategy.
- During the `v0.2.1` release smoke check, local service was not already
  running on `127.0.0.1:8787`; after starting
  `cargo run -- serve --web-dir ./web/dist`, `/` and `/v1/health` returned
  HTTP 200 and health returned `{"status":"ok","version":"0.2.1"}`. The
  verification server was then stopped, and no listener remained on 8787.
- `docs/super-mode/super-mode-project-methodology.pdf` and its HTML source
  exist as the generated Super Mode sharing artifact.

## Source Of Truth Documents

Read these in order:

1. `docs/dispatcher-2.0/PROJECT_MANUAL.md`
2. `docs/dispatcher-2.0/08-super-mode-handoff-2026-06-18.md`
3. `docs/dispatcher-2.0/00-pm-brief.md`
4. `docs/dispatcher-2.0/06-cross-check-report.md`
5. `docs/dispatcher-2.0/07-multi-agent-pm-followup.md`
6. `docs/dispatcher-2.0/09-project-strategy-and-roadmap-2026-06-18.md`
7. `docs/superpowers/plans/2026-06-17-dispatcher-2-followup-phases.md`
8. `docs/superpowers/plans/2026-06-16-codex-emergency-handoff.md`

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

2.0 emergency data path:

```text
native Codex response status/headers
-> QuotaSignal::from_response
-> quota_events row
-> EmergencyHandoffInput::build
-> handoff_packages row
-> /v1/telemetry latest_handoff
-> dashboard display
```

Post-`v0.2.0` planned / continuation data paths:

```text
native Codex reliable rate-limit headers
-> quota_snapshots row
-> threshold gate (DISPATCHER_PLANNED_HANDOFF_THRESHOLD, default 0.10)
-> PlannedHandoffInput::build
-> handoff_packages row
-> optional background provider-auto continuation when DISPATCHER_HANDOFF_AUTO_CONTINUE=1
-> handoff_continuations row with source/status
-> simple dashboard saved continuation + explicit primary review
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
- `QuotaSnapshot`
- `PlannedHandoffInput`

`crates/dispatcher-server/src/telemetry.rs` contains:

- `QuotaEventRecord`
- `QuotaSnapshotRecord`
- `HandoffContinuationRecord`
- `record_quota_event`
- `record_quota_snapshot`
- `record_handoff_package`
- `record_handoff_continuation`
- telemetry JSON fields:
  - `latest_quota_event`
  - `latest_quota_snapshot`
  - `latest_handoff`
  - `latest_handoff_continuation`

## Next Steps

Immediate:

- Do not push to `origin main`.
- Dispatcher 2.0 follow-up release `v0.2.1` has been published to `v2`.
- Keep `origin` as the Dispatcher 1.0 repository and continue to use only `v2`
  for Dispatcher 2.0 publication unless the user explicitly changes the
  strategy.
- Rerun the full verification matrix if any code, docs, or release
  configuration changes before the next commit or publication.
- Next product planning should start as a separate post-`v0.2.1` scope. The
  likely `v0.3.0` track is fallback worker certification for Codex handoff,
  not provider expansion as an end in itself and not part of the `v0.2.1`
  follow-up release.
- Current `v0.3.0` implementation target:
  - Keep Dispatcher 2.0 positioned as Codex Handoff Router.
  - Certify fallback workers at model level.
  - Apply eligibility filtering only to tagged handoff continuation.
  - Keep generic `provider-auto` routing available outside handoff.
  - Do not claim fallback models are equivalent to native Codex.
- Latest local `v0.3.0` verification result on 2026-06-18:
  - `./scripts/check-open-source-readiness.sh`: passed.
  - `cargo fmt --all --check`: passed.
  - `cargo clippy --workspace --all-targets -- -D warnings`: passed.
  - `cargo test --workspace`: passed.
  - `cargo check --workspace`: passed.
  - `pnpm --dir web format:check`: passed.
  - `pnpm --dir web typecheck`: passed.
  - `pnpm --dir web build`: passed.
  - `git diff --check`: passed.
  - Service smoke check: `/v1/health` returned HTTP 200 with
    `{"status":"ok","version":"0.3.0"}` and `/v1/providers` exposed
    `handoff_certification` for provider models.
  - Temporary verification server was stopped and no listener remained on
    `127.0.0.1:8787`.
- Strategic direction after `v0.2.1` is recorded in
  `docs/dispatcher-2.0/09-project-strategy-and-roadmap-2026-06-18.md`.
  The key decision is that `v0.3.0` should be a fallback worker certification
  layer for Dispatcher 2.0, not a pivot to a generic model switcher.

Completed post-`v0.2.0` follow-up phases in the current workspace:

- Planned handoff from reliable quota snapshots.
- Explicitly configured background fallback execution.
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

Latest final verification result for the `v0.2.1` release on 2026-06-18:

- `./scripts/check-open-source-readiness.sh`: passed.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
- `cargo check --workspace`: passed.
- `pnpm --dir web format:check`: passed.
- `pnpm --dir web typecheck`: passed.
- `pnpm --dir web build`: passed.
- `git diff --check`: passed.
- Local service smoke check: `/` returned HTTP 200; `/v1/health` returned
  HTTP 200 with `{"status":"ok","version":"0.2.1"}`.
- Post-release remote checks: `v2/main` contains release preparation commit
  `2fa536f`; `v0.2.1` tag exists on `v2`; GitHub Release `v0.2.1` exists.

Targeted release-boundary review on 2026-06-18:

- `DISPATCHER_HANDOFF_AUTO_CONTINUE=0` remains the example default.
- Background continuation records `source = background_auto` only through
  explicit truthy configuration.
- Planned handoff still comes only from reliable upstream rate-limit header
  pairs.
- README / dashboard wording treats fallback output as degraded and does not
  claim fallback equivalence to Codex-native execution.

## Subagent Management Rules

Use subagents, but keep them boxed in:

- One implementation task per worker.
- Do not dispatch parallel implementation workers that write overlapping files.
- Give each worker exact allowed files.
- After each worker returns, inspect the diff before continuing.
- If a worker adds Claude Code, planned 10%, automatic fallback execution, broad refactors, or unrelated UI, reject and correct it.

## Product Acceptance Criteria For This Follow-up

The milestone is done only when:

- A simulated native Codex `429` can still create a persisted emergency handoff package.
- Reliable native Codex rate-limit header pairs persist quota snapshots and can create a planned handoff package at the configured threshold.
- `DISPATCHER_HANDOFF_AUTO_CONTINUE` is disabled by default and records `source = background_auto` only when explicitly enabled.
- Tagged non-streaming and streaming provider-auto continuations persist terminal success/failure continuation telemetry.
- `/v1/telemetry` returns `latest_quota_snapshot`, `latest_handoff`, and `latest_handoff_continuation`.
- Dashboard shows planned/emergency handoff state, saved fallback continuation state, and explicit primary-route review actions without automatic provider switching.
- Existing Codex native routing behavior remains unchanged except for telemetry, diagnostic headers, and opt-in background continuation.
- Existing provider-auto behavior remains unchanged.

## Current User Intent

The user explicitly said:

```text
最后指挥子agent干出来的活不要偏离我们最开始设定的就好。
必须要有一个项目说明书，如果我要重开窗口继续做这个项目，可以直接读取进度，且之后怎么操作，进行到哪一步了，之后的几步是什么样子。
```

So this manual must stay current whenever a task completes.
