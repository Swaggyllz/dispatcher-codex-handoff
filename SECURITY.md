# Security Policy

## Supported Versions

During the alpha period, only the latest published release receives security fixes.

## Reporting a Vulnerability

Do not open a public issue for credentials exposure, authentication bypass, request
smuggling, arbitrary code execution, or other security-sensitive findings.

Use the repository's private security advisory form. Include:

- affected version or commit
- reproduction steps
- expected and actual behavior
- impact assessment
- any proposed mitigation

The maintainers aim to acknowledge reports within 72 hours and provide an initial
assessment within 7 days.

## Credential Model

Dispatcher reads provider credentials from the service process environment. It must
not log keys or forward Codex client credentials to third-party providers. Reports
that contradict this boundary are treated as high priority.
