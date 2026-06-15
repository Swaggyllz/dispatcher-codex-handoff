# Contributing to Dispatcher

Thanks for helping improve Dispatcher.

## Development Setup

Requirements:

- Rust 1.95+
- Node.js 22
- pnpm 10

```bash
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
cargo run -- serve --web-dir ./web/dist
```

## Required Checks

Run these before opening a pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
pnpm --dir web format:check
pnpm --dir web typecheck
pnpm --dir web build
```

## Pull Requests

- Keep each pull request focused on one behavior.
- Add regression tests for bug fixes.
- Do not include credentials, runtime databases, user configuration, or generated builds.
- Update documentation when behavior or configuration changes.
- Use Conventional Commit messages such as `fix(responses): tolerate null tool deltas`.

AI-assisted contributions are welcome, but contributors remain responsible for
understanding, reviewing, and testing every submitted change.

Security vulnerabilities must be reported privately according to [SECURITY.md](SECURITY.md).
