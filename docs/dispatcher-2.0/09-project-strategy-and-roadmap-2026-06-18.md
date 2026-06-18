# Dispatcher Project Strategy And Roadmap

Date: 2026-06-18

## Executive Decision

Dispatcher should not become a generic model switcher or a broad LLM gateway
positioned around "more models".

The durable product direction is:

```text
Codex Handoff Router
```

The core job is to preserve task continuity when the native Codex route is
under quota pressure, rate-limited, or otherwise unavailable. Provider routing,
domestic model support, Claude Code compatibility, and OpenAI-compatible APIs
are supporting infrastructure. They are not the product center.

The next strategic track should be:

```text
v0.3.0 = fallback worker certification layer for Dispatcher 2.0
```

This is a semantic-version milestone inside the Dispatcher 2.0 product line.
It is not a product pivot to "Dispatcher 3.0".

## Source Facts

Repository and release state on 2026-06-18:

- Local branch: `main`, tracking `v2/main`.
- Dispatcher 1.0 repository remains `origin`:
  `https://github.com/Swaggyllz/dispatcher.git`.
- Dispatcher 2.0 repository is `v2`:
  `https://github.com/Swaggyllz/dispatcher-codex-handoff.git`.
- Published release:
  `https://github.com/Swaggyllz/dispatcher-codex-handoff/releases/tag/v0.2.1`.
- Latest released product line: Dispatcher 2.0 / Codex Handoff Router.

Internal project facts:

- `README.md` leads with Codex-first handoff routing, emergency handoff
  packages, quota telemetry, and user-approved fallback continuation.
- `PROJECT_MANUAL.md` defines non-negotiable boundaries:
  Codex first, no exact quota promise without reliable headers, no default
  automatic fallback, no hosted Responses tool emulation, no hidden reasoning
  migration, and fallback models as degraded execution workers.
- `crates/dispatcher-server/src/handoff.rs` implements reliable rate-limit
  header parsing, quota snapshots, planned handoff gating, and
  `dispatcher_handoff.v1` packages.
- `crates/dispatcher-server/src/routes/responses.rs` separates native Codex
  `auto` from provider fallback `provider-auto`, records handoff telemetry,
  requires explicit truthy background fallback configuration, and preserves
  handoff continuation state.
- `crates/dispatcher-providers` already contains provider integrations and
  metadata for OpenAI, Anthropic, Gemini, OpenRouter, SiliconFlow, DeepSeek,
  Xiaomi MiMo, Ollama, and the demo provider.

External OpenAI/Codex facts checked against official documentation:

- Codex supports custom model providers configured with base URL, wire API,
  authentication, and HTTP headers.
- Codex can point at models/providers that support Chat Completions or
  Responses APIs, with Chat Completions support marked deprecated for future
  Codex releases.
- OpenAI documents an Amazon Bedrock provider path where the OpenAI-hosted
  Responses API is not in the request path and Bedrock provides an
  OpenAI-compatible Responses API implementation for supported OpenAI models.

Official references:

- `https://developers.openai.com/codex/config-advanced`
- `https://developers.openai.com/codex/config-reference`
- `https://developers.openai.com/codex/amazon-bedrock`

Strategic implication:

```text
"Can connect models" is becoming table stakes.
"Can govern Codex task handoff safely" is the defensible product value.
```

## What Was Done Right

### 1. The Product Moved From Routing To Continuity

Dispatcher 1.0 was fundamentally an intelligent routing and compatibility
layer: classify the request, choose a provider/model, track cost and health,
and return compatible responses.

Dispatcher 2.0 made the important product move: it reframed the problem as
agent continuity. The user pain is not only "which model should answer?" The
real pain is:

- Codex is in the middle of a real coding task.
- The route hits 429, quota pressure, or rate limits.
- The user does not want to reconstruct context manually.
- Any fallback must be visible, bounded, and reversible.

That is a stronger product thesis than a generic model switcher.

### 2. The Release Avoided False Claims

The implementation correctly avoids these traps:

- It does not promise an exact account quota percentage.
- It treats normalized headroom as reliable only when upstream
  limit/remaining header pairs exist.
- It does not claim fallback models are equivalent to native Codex.
- It does not migrate hidden reasoning state.
- It does not emulate hosted Responses tools in third-party providers.
- It does not silently switch users away from Codex by default.

This is the correct trust posture.

### 3. The Fallback Boundary Is Explicit

`provider-auto` is useful, but lossy. The current docs and UI describe fallback
as degraded execution. That wording should remain. It prevents product drift
and keeps users from confusing fallback output with native Codex completion.

### 4. The Engineering Foundation Is Real

The project now has a full working chain:

```text
native Codex route
-> quota signal and quota snapshot persistence
-> emergency or planned handoff package
-> handoff telemetry
-> dashboard display
-> explicit provider-auto continuation
-> saved continuation state
-> primary Codex review/reclaim workflow
```

The release also has a durable manual, release notes, validation commands, and
repository separation between Dispatcher 1.0 and Dispatcher 2.0.

### 5. The Release Boundary Was Correct

Keeping Dispatcher 1.0 on `origin` and publishing Dispatcher 2.0 through `v2`
was the right operational decision. It preserves the old project while letting
the new product direction stand on its own.

## What Is Still Weak

### 1. Positioning Can Still Drift

The codebase supports multiple providers and multiple client surfaces. That is
useful, but it creates a messaging hazard. If README, roadmap, or UI lead with
"supports many providers", the project starts to look like a generic model
switcher.

Future docs should use this hierarchy:

1. Codex handoff continuity.
2. Fallback worker execution.
3. Provider compatibility.
4. General routing as infrastructure.

### 2. Planned Handoff Is Still Conservative

The current planned handoff path creates a package from reliable observable
quota state. It does not yet ask the primary Codex route to produce a rich
`strong_summary` before failure.

That is acceptable for `v0.2.1`, but it is not the final planned-handoff
product.

### 3. Domestic Model Capability Needs Certification

Provider-level support is not enough. Model fleets are heterogeneous, and
aggregators can expose models with inconsistent tool behavior.

Future domestic model support must be model-level and evidence-backed:

- function calling behavior
- streaming behavior
- long-context behavior
- code-edit task stability
- tool-call argument correctness
- failure style and recovery behavior
- cost and latency under real handoff prompts

No model should become a default handoff worker just because its provider is
configured.

### 4. Real Codex Quota Signals Need Field Evidence

The implementation is conservative, but field behavior still matters. The
project needs observed samples from ChatGPT-authenticated Codex sessions and
API-key-authenticated sessions. Without this, planned handoff thresholds remain
technically correct but operationally uncertain.

### 5. Packaging Is Still Alpha

The project is source-build first. README says release binaries are unsigned,
multi-user auth is absent, and provider metadata is not a billing guarantee.
That is honest, but the project needs packaging work before it can be a broadly
usable tool.

## Strategic Product Definition

### Product Name

```text
Dispatcher 2.0: Codex Handoff Router
```

### One-Line Positioning

```text
When Codex primary capacity becomes unreliable, Dispatcher turns the task into
an auditable handoff package and routes degraded continuation work to certified
fallback workers without pretending the fallback is Codex.
```

### Product Promise

Dispatcher helps users continue coding tasks under quota pressure with less
manual reconstruction and more operational trust.

### Product Non-Promise

Dispatcher does not make third-party models equivalent to Codex. It does not
move hidden context. It does not bypass quota. It does not silently switch the
user's model.

## Version Strategy

### `v0.2.x`: Stabilize The Published 2.0 Foundation

Goal:

```text
Make the released Codex handoff router trustworthy and easy to validate.
```

Recommended scope:

- Collect real quota/rate-limit header samples.
- Add targeted fixtures for observed header variants.
- Improve dashboard wording for planned vs emergency handoff.
- Add a release binary/package story.
- Add a "how to test handoff locally" guide.
- Keep provider expansion frozen unless it directly supports handoff testing.

Acceptance criteria:

- A user can reproduce emergency handoff locally.
- A user can inspect the latest handoff and continuation in the dashboard.
- The docs explain exactly what is and is not automatic.
- No default behavior routes away from Codex without explicit user/operator
  approval.

### `v0.3.0`: Fallback Worker Certification Layer

Goal:

```text
Turn configured providers into certified handoff workers with model-level
evidence and clear routing policy.
```

This is the correct next major milestone. It should not be called or framed as
a generic model switcher.

Included:

- Model-level handoff capability profiles.
- A handoff-worker eval harness.
- Built-in evaluation fixtures for `dispatcher_handoff.v1` prompts.
- Certification labels such as:
  - `handoff_text_only`
  - `handoff_code_patch`
  - `handoff_tool_capable`
  - `handoff_long_context`
  - `not_certified`
- Dashboard display of fallback worker certification state.
- Routing policy that prefers certified workers for handoff continuation.
- Documentation that domestic models are fallback workers, not Codex
  replacements.

Not included:

- Generic model switching as the main product.
- Claude Code-first behavior.
- Default silent model switching.
- Hosted tool emulation.
- Claims of Codex equivalence.

Recommended first worker candidates:

- DeepSeek for code/text fallback where tool behavior is validated.
- Xiaomi MiMo for long-context summarization or text-only fallback until tool
  support is proven.
- SiliconFlow/Qwen/OpenRouter only after model-level certification fixtures
  pass reliably.
- Ollama for local/demo flows, not as a default serious fallback unless a
  specific local model is certified.

Acceptance criteria:

- Each default fallback candidate has a model-level capability profile.
- The eval harness can run handoff prompt fixtures against configured
  providers.
- Provider-auto handoff continuation chooses certified workers before generic
  cheap/fast candidates.
- Dashboard shows why a model is or is not eligible for handoff continuation.
- README and project manual still describe Dispatcher as Codex Handoff Router.

### `v0.4.0`: Strong Planned Handoff

Goal:

```text
When native Codex still has enough capacity, ask Codex to produce a strong
handoff summary before the route fails.
```

Included:

- Primary-route generated `strong_summary` packages.
- Distinct confidence labels for:
  - `strong_summary`
  - `observable_reconstruction`
  - `emergency_reconstruction`
- User-visible comparison between primary summary and fallback continuation.
- Guardrails for stale or contradictory handoff summaries.

Acceptance criteria:

- Planned handoff can produce richer task state than emergency reconstruction.
- Emergency handoff remains available when planned handoff cannot run.
- UI makes confidence differences obvious.

### `v0.5.0`: Operator And Team Hardening

Goal:

```text
Make Dispatcher useful for real teams and repeated operational use.
```

Included:

- Signed or checksummed release artifacts.
- Policy templates for fallback permission.
- Budget and cost controls for fallback routes.
- Exportable handoff packages.
- Audit views for handoff decisions.
- Better onboarding for local-only use.

Acceptance criteria:

- A team can configure who may enable background fallback.
- A team can audit when fallback happened and why.
- A user can install and smoke-test without reading the whole codebase.

## The `v0.3.0` Product Contract

The next milestone must obey this contract:

```text
v0.3.0 certifies fallback workers for Codex handoff.
It does not turn Dispatcher into a generic model switcher.
```

### Allowed Work

- Add model-level certification metadata.
- Add eval fixtures for handoff prompts.
- Add model eligibility filters for handoff continuation.
- Add dashboard visibility for fallback worker quality.
- Add docs explaining which worker is suitable for which handoff class.
- Improve provider metadata freshness and override flow.

### Forbidden Work

- Repositioning the project as a generic model switcher.
- Repositioning around Claude Code.
- Adding provider support without a handoff use case.
- Making provider-auto the default Codex path.
- Auto-switching without explicit configuration.
- Claiming fallback is equivalent to Codex.
- Building broad hosted tool emulation.

## Architecture Direction

The architecture should preserve three lanes:

```text
Lane 1: native Codex auto
  - preserve Responses request shape
  - select Codex model/effort/speed
  - observe quota and route health

Lane 2: handoff package and telemetry
  - record quota events and snapshots
  - build planned or emergency handoff packages
  - persist handoff and continuation state

Lane 3: certified provider-auto fallback
  - convert handoff prompt into fallback execution request
  - route only to eligible certified workers
  - record result
  - send result back for primary Codex review
```

Avoid merging these lanes into a single "smart router" abstraction. The product
needs the lanes to remain visible because user trust depends on knowing when
they are in native Codex, in handoff packaging, or in degraded fallback.

## Decision Records

### Decision 1: Domestic Models Are Workers, Not Replacements

Domestic providers are useful because they can keep work moving when Codex is
unavailable or expensive. They are not positioned as equivalent Codex-native
agents.

This means future docs should say:

```text
certified fallback worker
```

not:

```text
Codex replacement
```

### Decision 2: Provider-Auto Remains Degraded Execution

`provider-auto` should stay explicit and labeled. Its job is to execute a
bounded continuation prompt, not to impersonate the full Codex route.

### Decision 3: OpenAI Provider Expansion Raises The Bar

Official Codex support for custom providers and Bedrock-style provider paths
means Dispatcher should not compete on "Codex can talk to another endpoint."

Dispatcher should compete on:

- handoff package quality
- continuation evidence
- fallback worker certification
- primary route reclaim
- local operator control

### Decision 4: Documentation Is A Product Control Surface

The project manual is not just notes. It is the mechanism that prevents
future agents from drifting into the wrong product. Every milestone should
update it.

## Immediate Next Actions

1. Create a `v0.3.0` implementation plan focused only on fallback worker
   certification.
2. Add an explicit handoff-worker metadata schema proposal before changing
   provider routing.
3. Define 5-8 handoff eval fixtures:
   - text-only continuation
   - code patch continuation
   - tool-call style continuation
   - long-context summary continuation
   - failed-provider recovery
   - contradiction detection
   - primary-review prompt generation
4. Decide which models are candidates for default certification.
5. Update README language only after certification behavior exists.
6. Keep `origin` untouched and use `v2` for Dispatcher 2.0 publication.

## Final Verdict

Dispatcher has made the correct strategic move:

```text
from model routing
to Codex task continuity
```

The project should now protect that move. More providers are useful only when
they become certified fallback workers inside the Codex handoff workflow.

The future is not:

```text
universal model switcher
```

The future is:

```text
observable, user-approved, evidence-backed Codex handoff and recovery
```
