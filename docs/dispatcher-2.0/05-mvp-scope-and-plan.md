# MVP Scope And Plan

## MVP Name

```text
Codex Handoff Router
```

## Goal

Let Codex users continue a coding task through a configured fallback model when the primary Codex route is quota-constrained or rate-limited.

## Scope

Included:

- Codex `/v1/responses` only.
- Native `auto` as primary route.
- `provider-auto` as fallback route.
- Emergency handoff on `429` or quota-like error.
- Planned handoff only when reliable quota headers are captured.
- Manual handoff for testing and user control.
- DeepSeek as first default domestic tool-capable fallback.
- Handoff package persistence and dashboard visibility.

Excluded:

- Claude Code.
- Hosted Responses tool emulation.
- Hidden reasoning migration.
- Automatic quality equivalence claims.
- Broad provider marketplace support.

## Proposed Implementation Phases

### Phase 1: Research Hardening

- Add docs under `docs/dispatcher-2.0/`.
- Convert this research into an implementation plan.
- Define exact tests for Codex provider-auto tool loops.

### Phase 2: Quota State Foundation

- Capture rate-limit and request-id headers from native Codex upstream calls.
- Add structured quota event and latest quota snapshot storage.
- Keep quota state separate from circuit breaker state.
- Expose quota state through telemetry API.

### Phase 3: Emergency Handoff

- Detect `429` and quota-like errors from native Codex path.
- Create `dispatcher_handoff.v1` package with `emergency_reconstruction`.
- Persist handoff package.
- Surface handoff package in dashboard.
- Allow user to copy continuation prompt.

### Phase 4: Planned Handoff

- When reliable headers show headroom at or below threshold, ask primary Codex route to produce `strong_summary` handoff.
- Store the package.
- Require user approval before switching fallback route.

### Phase 5: Fallback Execution

- Route fallback continuation through `provider-auto`.
- Start with DeepSeek for tool-capable tasks.
- Apply execution guardrails to fallback prompt.
- Add tests for function tool round trips.

### Phase 6: Review And Recovery

- When primary Codex route becomes available again, show a review package:
  - handoff package
  - fallback model
  - changed files
  - commands run
  - verification status

## First Implementation Plan Boundary

The first implementation plan should cover Phases 2 and 3 only:

- structured quota signals
- emergency handoff package
- API/dashboard visibility

Planned handoff and automatic fallback execution should come after emergency handoff is observable and testable.

## Implementation Status

Phase 2/3 first slice:

- Structured quota event storage is implemented for native Codex emergency signals.
- Emergency `dispatcher_handoff.v1` package persistence is implemented.
- Dashboard telemetry shows the latest emergency handoff package.

Still pending:

- Planned 10% handoff from reliable quota snapshots.
- Automatic fallback execution through `provider-auto`.
- Primary-route recovery review workflow.

## Acceptance Criteria

- A simulated native Codex `429` creates a persisted handoff package.
- The handoff package is marked `emergency_reconstruction`.
- Dashboard/telemetry can show the latest package.
- Existing Codex native routing tests still pass.
- Existing provider-auto tests still pass.
- No hosted-tool compatibility claims are added.
