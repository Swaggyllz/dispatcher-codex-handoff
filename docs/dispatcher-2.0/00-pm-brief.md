# Dispatcher 2.0 PM Brief

Date: 2026-06-16

## Product Thesis

Dispatcher 2.0 should move from "intelligent model routing" to "agent continuity routing" for Codex-first coding workflows.

The first target is Codex, not Claude Code. Codex is already wired through Dispatcher's `/v1/responses` entrypoint, and the project already has two important modes:

- `auto`: native Codex/OpenAI Responses forwarding with Dispatcher-controlled model, reasoning effort, and speed.
- `provider-auto`: lossy Responses-to-provider compatibility for configured third-party providers.

The 2.0 opportunity is to detect when the primary Codex model is under quota pressure or blocked by rate limits, package the current task state, and continue through a configured fallback model in a constrained execution mode.

## Positioning

Working name:

```text
Dispatcher 2.0: Codex Handoff Router
```

One-line positioning:

```text
When Codex primary capacity runs out, Dispatcher turns the current task into a bounded handoff package and routes continuation work to a fallback model without starting from zero.
```

## Non-Goals For MVP

- Do not promise exact official account quota percentages unless the upstream exposes reliable headers.
- Do not emulate OpenAI hosted Responses tools in third-party providers.
- Do not claim domestic models are equivalent to Codex native models.
- Do not solve Claude Code first.
- Do not attempt full hidden-context or reasoning-state migration.

## MVP Decision

Start with a Codex-first emergency handoff and a narrow planned-handoff path:

1. Emergency trigger:
   - A native Codex upstream call returns `429`, quota-specific error content, or `retry-after`.
   - Dispatcher creates an emergency package from observable request/session/workspace state.
   - Fallback model continues in guarded mode.

2. Planned trigger:
   - Only enabled when upstream response headers expose reliable limit/remaining/reset values.
   - Trigger when normalized headroom is at or below the configured threshold, default `10%`.
   - The primary model is asked to produce a structured handoff package before hard failure.

3. Manual trigger:
   - User can request handoff explicitly from the dashboard or config-driven mode.
   - This is useful for validating the workflow before quota events are reliable.

## Success Criteria

- A Codex user can continue a tool-using coding task after primary model quota pressure without manually rebuilding all context.
- Dispatcher clearly labels handoff confidence as `strong_summary` or `emergency_reconstruction`.
- Fallback execution is constrained and observable.
- DeepSeek is the first default domestic agentic fallback; SiliconFlow/Qwen joins only after model-level tool capability validation.
- Dashboard shows handoff status, trigger, confidence, selected fallback, and next recommended step.

## Recommended Workstreams

- Protocol: lock the Codex native vs provider-auto contract.
- Quota: add structured quota events and snapshots.
- Compatibility: validate fallback models against function-calling and streaming behavior.
- Handoff: implement a compact schema and guardrail prompt.
- UX: expose a dashboard panel that sets user expectations.

