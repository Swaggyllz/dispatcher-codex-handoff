# Handoff Schema

## Schema

Minimal schema version:

```text
dispatcher_handoff.v1
```

Recommended JSON shape:

```json
{
  "schema_version": "dispatcher_handoff.v1",
  "package_id": "handoff_...",
  "created_at": "2026-06-16T00:00:00Z",
  "trigger": "planned",
  "confidence": "strong_summary",
  "objective": "",
  "latest_user_request": "",
  "current_status": "in_progress",
  "completion_criteria": [],
  "workspace": {
    "cwd": "",
    "repo_name": "",
    "branch": "",
    "dirty_state": "unknown",
    "touched_files": [],
    "relevant_files": []
  },
  "execution_state": {
    "mode": "edit_allowed",
    "last_successful_step": "",
    "next_recommended_step": "",
    "blocked_on": "",
    "commands_run": [],
    "verification_run": []
  },
  "technical_context": {
    "key_findings": [],
    "decisions_made": [],
    "assumptions": [],
    "constraints": []
  },
  "routing_context": {
    "agent_tier": "medium",
    "requested_model": "",
    "selected_model": "",
    "reasoning_effort": "medium",
    "speed": "standard",
    "dispatcher_mode": "auto"
  },
  "continuation_prompt": "",
  "hazards": [],
  "open_questions": []
}
```

## Trigger Values

Allowed trigger values:

- `planned`
- `quota_warning`
- `rate_limit_429`
- `manual`

Allowed confidence values:

- `strong_summary`
- `emergency_reconstruction`

## Planned Handoff Contract

When primary capacity is still available, Dispatcher should ask the primary model to produce the handoff package.

The prompt contract:

```text
You are preparing a handoff for a fallback coding model.

Return a compact dispatcher_handoff.v1 package.
Separate facts from assumptions.
Name exact files and commands only when known.
Do not include hidden chain-of-thought.
Write a continuation_prompt that a weaker model can follow safely.
Prefer small atomic next steps.
Clearly state what must not be changed.
```

## Emergency Handoff Contract

After a hard quota failure, Dispatcher may not have a strong-model summary. It should synthesize from observable state only:

- latest user message
- current working directory
- branch
- git status
- recently touched files
- last route metadata
- last command records, if available
- final error status and retry-after, if available

Emergency handoff must set:

```json
{
  "confidence": "emergency_reconstruction",
  "technical_context": {
    "assumptions": [
      "Previous model was interrupted before producing a handoff summary.",
      "State was reconstructed from observable workspace and routing data only."
    ]
  }
}
```

## Fallback Execution Guardrails

Fallback models should receive explicit guardrails:

- Default to `research_only` or `verify_only` unless `mode` is `edit_allowed`.
- Re-read relevant files before editing.
- Do not perform broad refactors.
- Do not run destructive git commands.
- Do not create commits, branches, pull requests, or releases unless explicitly requested.
- Treat `hazards` as hard constraints.
- If confidence is `emergency_reconstruction`, first audit state before editing.
- Stop and report if the task diverges from the handoff package.

## Dashboard Fields

Show:

- handoff status
- trigger
- created time
- confidence
- current status
- objective
- next recommended step
- execution mode
- selected fallback model
- relevant file count
- touched file count
- verification state
- hazard count
- open question count
- last error status
- copyable continuation prompt

