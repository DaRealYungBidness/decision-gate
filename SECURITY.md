# Security

Decision Gate is built with a Zero Trust posture. Inputs are treated as hostile
and disclosure decisions fail closed on missing or unverifiable evidence.

See `Docs/security/threat_model.md` for the full threat model, trust boundaries,
and failure posture. Operational controls are documented in
`Docs/guides/security_guide.md`.

If you believe you have found a security issue, please report it privately to
`support@assetcore.io` with the subject line: `DG SECURITY`.

Please do not open public issues for security vulnerabilities.

Include as much of the following as you can:

- A concise summary of the issue and impact.
- Steps to reproduce (minimal if possible).
- Affected versions or git commit.
- Relevant configuration (redact secrets).
- Any proof-of-concept or logs that help confirm the issue.

I will acknowledge reports when I can; response time is best-effort.

## Supply Chain & Compliance Roadmap

- SBOM/provenance are planned but not yet provided.
- FIPS-validated crypto is not currently supported; it is on the long-term roadmap.
