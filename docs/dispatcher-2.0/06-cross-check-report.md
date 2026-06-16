# Cross-Check Report

## Agreement Across Agents

All research threads support a Codex-first Dispatcher 2.0 direction.

The strongest shared conclusions:

- Codex is the right first target because `/v1/responses` already exists.
- `provider-auto` already makes domestic fallback possible, but it is lossy.
- The "10%" concept must be normalized headroom, not an official account balance promise.
- Quota state should be separate from provider health and circuit breaker.
- DeepSeek is the safest first domestic fallback for tool-using tasks.
- Handoff should be a task package, not a transcript migration.

## Tensions Found

### Planned Handoff Needs Headers

The product wants "handoff before exhaustion", but the current code does not capture the headers required to do this reliably.

Decision:

```text
Emergency handoff first. Planned 10% handoff only after quota snapshots exist.
```

### Provider-Auto Is Useful But Not Native Codex

Provider-auto can bridge text and function tools, but it does not emulate hosted tools, full Responses streams, or reasoning summaries.

Decision:

```text
Market provider-auto continuation as degraded execution mode.
```

### Model Metadata Can Overpromise Tools

Some providers aggregate heterogeneous model fleets. Provider-level tool support is not enough for agentic fallback.

Decision:

```text
Use model-level allowlists before adding SiliconFlow/OpenRouter to default agentic fallback.
```

### Emergency Reconstruction Can Sound Too Confident

After 429, Dispatcher may only know observable workspace/request state.

Decision:

```text
Every emergency package must show confidence = emergency_reconstruction and include assumptions.
```

## Recommended MVP Cut

Build this first:

```text
Native Codex 429 -> emergency handoff package -> dashboard visibility -> copyable continuation prompt
```

Then build:

```text
quota snapshots -> planned 10% handoff -> fallback execution through provider-auto
```

This sequence gives users value quickly without pretending the hard parts are solved.

## Open Questions Before Implementation

- Persist handoff packages in SQLite only, or also write JSON files under a local data directory?
- Should emergency handoff automatically retry with fallback, or require user approval first?
- Which Codex headers are reliably available in ChatGPT subscription mode vs OpenAI API key mode?
- What is the default soft budget when no upstream quota headers exist?
- Should dashboard support editing the continuation prompt before fallback execution?

## PM Recommendation

Define Dispatcher 2.0 as a product direction now, but release it incrementally:

- `v0.2.x`: Codex emergency handoff research and foundation.
- `v0.3.x`: planned headroom detection and dashboard workflow.
- `v0.4.x`: guarded fallback execution and review recovery.

Do not wait for a perfect "seamless" system. The first valuable product is honest, observable, and user-approved handoff.

