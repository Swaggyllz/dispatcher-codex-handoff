# Quota Detection Research

## Existing Signals

Dispatcher already detects coarse provider failures:

- `RateLimited`
- `AuthFailed`
- `Timeout`
- `Network`
- `Other`

Provider implementations convert HTTP `429` into `ProviderError::RateLimited`, but the error only carries a string. Response headers are currently discarded.

Telemetry currently stores:

- provider and model
- token usage
- latency
- estimated cost
- success flag
- error message
- routing strategy
- agent tier
- fallback flag

Codex route telemetry stores:

- requested model
- selected model
- reasoning effort
- speed
- agent tier
- reason
- success
- status code
- latency
- error message

## Missing For "10% Remaining"

Dispatcher cannot currently calculate precise remaining primary capacity because it does not persist:

- response rate-limit headers
- `retry-after`
- reset timestamps
- request IDs on generic provider paths
- quota bucket identity
- structured error kind
- provider/model quota snapshots
- per-attempt route history as telemetry

`ProviderError::RateLimited` and `ChatCompletionResponse` do not carry structured quota metadata.

## Correct Product Language

Do not say:

```text
Codex has exactly 10% quota left.
```

Say:

```text
Dispatcher estimates primary model headroom is at or below the configured handoff threshold.
```

When upstream headers exist, the estimate can be precise for that exposed bucket. When headers do not exist, it is a local soft-budget estimate.

## Trigger Model

### Emergency Trigger

Emergency handoff starts when any of these occur:

- HTTP `429`
- quota-specific error body
- `retry-after`
- provider error classified as `RateLimited`

Emergency trigger should mark the provider/model/bucket as temporarily constrained and route away immediately.

### Planned Trigger

Planned handoff starts when normalized headroom is at or below threshold, default `10%`.

The normalized value should take the minimum of reliable exposed dimensions:

```text
min(
  requests_remaining / requests_limit,
  input_tokens_remaining / input_tokens_limit,
  output_tokens_remaining / output_tokens_limit
)
```

Only use a dimension when both `remaining` and `limit` are known and trustworthy.

### Estimated Trigger

When no upstream headers exist, Dispatcher can use local soft budgets:

- rolling request count
- rolling input/output token usage
- rolling estimated spend
- user-configured monthly/daily/session budget

Estimated trigger must be labeled as estimated.

## Architecture Recommendation

Add quota state separate from provider health and circuit breaker.

Provider health answers:

```text
Is this provider generally succeeding?
```

Quota state answers:

```text
Is this provider/model currently close to or inside a quota boundary?
```

Do not mix quota exhaustion into the generic circuit breaker. Quota exhaustion is often expected and reset-bound; generic failure state is not.

## Persistence Recommendation

Persist two data shapes:

1. Latest quota snapshot:
   - provider
   - model
   - bucket
   - limit
   - remaining
   - reset_at
   - source: header / error / estimate

2. Quota event:
   - timestamp
   - provider
   - model
   - status code
   - error kind
   - retry-after
   - normalized headroom
   - raw header names captured, without secrets

## Risks

- Header semantics vary by provider.
- OpenAI-compatible does not mean rate-limit-compatible.
- Some providers expose no useful headroom signal.
- Streaming usage and failure accounting need special care.
- Provider trait changes may touch every provider implementation.
- SQLite migrations must remain backward compatible.

