# Dispatcher 2.0 Super Mode Handoff

Date: 2026-06-18

## Scope Lock

Dispatcher 2.0 remains the Codex Handoff Router. The current milestone is
Codex-first and must not add Claude Code behavior, hosted Responses tool
emulation, hidden reasoning migration, or automatic fallback by default.

Release boundary:

- Do not touch or publish over the Dispatcher 1.0 repository.
- `origin` is the 1.0 repository: `https://github.com/Swaggyllz/dispatcher.git`.
- `v2` is the Dispatcher 2.0 repository:
  `https://github.com/Swaggyllz/dispatcher-codex-handoff.git`.
- Publishing was approved on 2026-06-18 for Dispatcher 2.0. Publish only to
  `v2` unless the user changes strategy.

## Current Git State

- Branch: `main`, tracking `v2/main`.
- `v0.2.1` release preparation commit:
  `2fa536f chore: prepare v0.2.1 release`.
- `v0.2.1` tag was pushed to `v2`.
- GitHub Release `v0.2.1` was created:
  `https://github.com/Swaggyllz/dispatcher-codex-handoff/releases/tag/v0.2.1`.
- This handoff update records the publication state after the release.

Primary modified areas:

- Planned quota snapshot handoff:
  `crates/dispatcher-server/src/handoff.rs`,
  `crates/dispatcher-server/src/telemetry.rs`,
  `crates/dispatcher-server/src/routes/responses.rs`.
- Background fallback continuation and streaming continuation persistence:
  `crates/dispatcher-server/src/routes/responses.rs`,
  `crates/dispatcher-server/src/routes/responses_compat.rs`.
- Primary reclaim UI:
  `web/src/components/SimpleDashboard.tsx`,
  `web/src/hooks/usePrimaryReview.ts`,
  `web/src/lib/api/dashboard.ts`,
  `web/src/types.ts`,
  `web/src/i18n/locales/en.json`,
  `web/src/i18n/locales/zh.json`,
  `web/src/index.css`.
- Documentation and config:
  `.env.example`, `README.md`, `README.zh-CN.md`,
  `docs/dispatcher-2.0/PROJECT_MANUAL.md`,
  `docs/dispatcher-2.0/07-multi-agent-pm-followup.md`,
  `docs/superpowers/plans/2026-06-17-dispatcher-2-followup-phases.md`,
  `docs/super-mode/`.

## Follow-up Status

The four post-`v0.2.0` follow-up phases are implemented and published as
`v0.2.1`:

- Planned handoff from reliable upstream rate-limit header pairs.
- Background provider-auto continuation, disabled by default and enabled only
  by `DISPATCHER_HANDOFF_AUTO_CONTINUE=1`.
- Tagged non-streaming and streaming provider-auto continuation persistence.
- Simple dashboard saved fallback state and explicit primary Codex review.

The implementation passed the full verification matrix again before the
`v0.2.1` release on 2026-06-18:

```bash
./scripts/check-open-source-readiness.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
git diff --check
```

## Service Check

Initial status:

- `127.0.0.1:8787` was not listening before the release smoke check.
- `curl --noproxy '*' http://127.0.0.1:8787/` could not connect before
  startup.
- `curl --noproxy '*' http://127.0.0.1:8787/v1/health` could not connect
  before startup.

Startup verification:

```bash
cargo run -- serve --web-dir ./web/dist
curl --noproxy '*' -i http://127.0.0.1:8787/
curl --noproxy '*' -i http://127.0.0.1:8787/v1/health
```

Result:

- `/` returned HTTP 200 and served the Dispatcher 2.0 web bundle.
- `/v1/health` returned HTTP 200 with
  `{"status":"ok","version":"0.2.1"}`.
- The verification server was stopped after the check.

## Super Mode Artifact

Generated sharing artifact:

- `docs/super-mode/super-mode-project-methodology.html`
- `docs/super-mode/super-mode-project-methodology.pdf`

PDF extraction evidence:

- File size: 659,322 bytes.
- Page count: 5.
- PDF title: `超级模式：把 AI 从聊天工具变成项目工程代理`.
- Extracted sections cover fact-source reading, acceptance criteria,
  Multi-agent PM decomposition, verify/fix loops, risk gates, and handoff
  documentation.

The artifact describes the Super Mode workflow: read facts first, lock scope,
define acceptance criteria, use Multi-agent PM for scoped decomposition, execute
in verify/fix loops, record risk, and leave a handoff trail.

## Final Local Checks In This Handoff Session

- `./scripts/check-open-source-readiness.sh`: passed.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
- `cargo check --workspace`: passed.
- `pnpm --dir web format:check`: passed.
- `pnpm --dir web typecheck`: passed.
- `pnpm --dir web build`: passed.
- `git diff --check`: passed.
- Release service smoke check: `/` returned HTTP 200 and `/v1/health` returned
  `{"status":"ok","version":"0.2.1"}`.
- Release remote checks passed: `v2/main` contains `2fa536f`; `v0.2.1` tag
  exists on `v2`; GitHub Release `v0.2.1` exists.
- Trailing-whitespace scan for new untracked text files: passed.
- `lsof -nP -iTCP:8787 -sTCP:LISTEN`: no listener after stopping the
  verification server.
- Targeted release-boundary review checked default-off background fallback,
  reliable-header-only planned handoff, and degraded fallback wording.

## Next Operator Steps

1. Keep this publication-record handoff update on `v2/main`.
2. Do not push to `origin`, delete generated artifacts, or rewrite history
   without explicit confirmation.
3. Treat the next product step as a new post-`v0.2.1` scope. The likely
   `v0.3.0` track is provider/model expansion and Codex-compatible routing
   policy.
4. If any code, docs, or release configuration changes, rerun the full
   verification matrix before the next commit or publication.
