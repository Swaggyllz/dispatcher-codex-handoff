# Dispatcher 2.0 Follow-up Multi-agent PM Report

Date: 2026-06-17

## Goal

Complete the four post-`v0.2.0` Dispatcher 2.0 phases without drifting from the
Codex-first product boundary:

1. Planned handoff from reliable quota snapshots.
2. Explicitly configured background fallback continuation.
3. Streaming continuation persistence.
4. Richer primary-route reclaim workflows.

## Acceptance Criteria

- Reliable Codex rate-limit header pairs are persisted as quota snapshots and
  can trigger a planned handoff warning when normalized headroom is at or below
  the configured threshold.
- Emergency handoff behavior remains unchanged for 429 / retry-after signals.
- Background fallback continuation is off by default and only runs when
  explicitly enabled by configuration.
- Tagged streaming provider-auto continuations persist completed output or
  terminal failure state; partial stream output is not persisted.
- Dashboard UI distinguishes emergency, planned, background, streaming, and
  primary-review states without claiming fallback equivalence to Codex.
- Existing native Codex and provider-auto behavior remains compatible.
- Full Rust and web verification passes before release.

## Multi-agent Division

Four explorer agents inspected independent domains:

- Quota / planned handoff: confirmed header parsing exists, but snapshot state,
  threshold gating, and planned package creation are missing.
- Background fallback: confirmed continuation is user-click only; automatic
  execution needs explicit opt-in, dedupe, and status/source telemetry.
- Streaming persistence: confirmed non-streaming continuation persistence
  exists; stream branch drops `handoff_package_id`.
- Primary reclaim / UI: confirmed review prompt exists but default simple UI
  does not expose a primary-review action.

## Product Decisions

- Do not implement Claude Code behavior in this phase.
- Do not claim official quota balance. Use "observed normalized headroom" from
  reliable headers only.
- Do not enable automatic fallback by default. Treat
  `DISPATCHER_HANDOFF_AUTO_CONTINUE=1` as explicit operator approval.
- Do not emulate hosted Responses tools for provider fallback.
- Do not migrate hidden reasoning or private context. Fallback uses only
  persisted package fields and observable prompts.
- Do not automatically switch back to Codex. Primary reclaim remains an
  explicit user action.

## Implementation Slices

### Slice 1: Quota Snapshots And Planned Handoff

Add quota snapshot records for reliable rate-limit header pairs. Add a pure
threshold gate with default `0.10`, configurable through
`DISPATCHER_PLANNED_HANDOFF_THRESHOLD`. When a non-emergency native Codex
response has reliable headroom at or below threshold, persist a planned handoff
package. The package is a conservative observable-state package; it must not
pretend to contain hidden reasoning.

### Slice 2: Background Fallback Continuation

Add `DISPATCHER_HANDOFF_AUTO_CONTINUE=1` opt-in. When enabled, emergency or
planned packages schedule one provider-auto non-streaming continuation in the
background. Persist source/status so the UI can distinguish user-clicked and
background continuations.

### Slice 3: Streaming Continuation Persistence

Thread `handoff_package_id` through the streaming provider-auto path. Persist
only terminal success text or terminal failure. Do not persist partial stream
deltas.

### Slice 4: Primary Reclaim UI

Expose saved continuation state in the simple dashboard. Add copy-review-prompt
and explicit "Review with Codex" actions. The review action sends the generated
review prompt through native Codex only after a user click.

## Implementation Result

Current workspace implementation status:

- `handoff.rs` now parses reliable quota snapshots, gates planned handoff by
  normalized threshold, and builds conservative planned packages.
- `telemetry.rs` now persists `quota_snapshots` and continuation
  `source` / `status`, with migration defaults for existing databases.
- `responses.rs` now records quota snapshots, creates planned packages, keeps
  emergency behavior intact, persists tagged streaming continuation terminal
  state, and supports opt-in background provider-auto continuation.
- `responses_compat.rs` exposes final accumulated stream text for terminal
  persistence.
- The simple dashboard now shows reliable quota snapshot data, saved fallback
  continuation state, continuation source/status, copy-review-prompt, and
  explicit primary-route review.
- `.env.example`, README files, and `PROJECT_MANUAL.md` document the new
  configuration and safety boundaries.

## Verification Matrix

Run before completion:

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

Result on 2026-06-17: all commands passed.
