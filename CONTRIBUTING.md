# Contributing

Thank you for the interest in this project. Decision Gate is a reflection of how I abstract/see the world, and I am honored to share that with you.

Decision Gate does not accept pull requests (including doc-only PRs). Issues are
the primary way to report problems, propose ideas, or start design discussion.

## Why no PRs

Decision Gate is intentionally built as a closed-form system with strict
determinism, security boundaries, and a small core. As a solo maintainer, I
cannot reliably validate intent or architecture alignment in unsolicited PRs.
I also want to keep the system aligned with its mathematical shape
(i.e. ret-logic -> DG core -> security model), which requires end-to-end reasoning.

If you see a problem, please open an issue instead. That helps me triage and
address it without compromising correctness or auditability.

## What issues are welcome

- Bug reports with concrete reproduction steps.
- Documentation corrections or clarity requests.
- Feature requests and design discussion (with clear use-cases).
- Security concerns (see the Security section below).

I read everything, but I cannot promise timelines or outcomes.

## Issue templates

If you open an issue in GitHub, please use the issue templates:

- Bug report: [`.github/ISSUE_TEMPLATE/bug_report.md`](.github/ISSUE_TEMPLATE/bug_report.md)
- Feature request: [`.github/ISSUE_TEMPLATE/feature_request.md`](.github/ISSUE_TEMPLATE/feature_request.md)

If you cannot use the templates, follow the same headings in the relevant file.

## Bug reports (what to include)

Please include as much of the following as you can:

- What you expected vs what happened.
- Steps to reproduce (minimal if possible).
- Decision Gate version or git commit.
- OS and environment details.
- Relevant config (redact secrets).
- Logs or error messages.

## Feature requests / design discussion

Decision Gate solves a specific, bounded problem. If you propose a change,
please include:

- The exact problem you are trying to solve.
- Why it cannot be solved with existing providers or schemas.
- Example evidence inputs/outputs or schemas.
- Any security or determinism tradeoffs you are aware of.

This helps me evaluate whether the request fits the core model.

## Current focus areas (Updated 2026-02-03 UTC)

If you want to help test, the current focus areas live in:

- `Docs/roadmap/README.md`
- `Docs/roadmap/foundational_correctness_roadmap.md`

When you open an issue with test results, include the focus area, exact steps,
and your environment details.

## Security

Please do not open public issues for security vulnerabilities. Follow the
instructions in `SECURITY.md` so disclosure can be handled responsibly.

## Scope and boundaries

Enterprise components live in a private repository and are not open for PRs.
If you have questions about enterprise features, open an issue and I will point
you to the right place.

## Response expectations

I do my best to respond, but there is no SLA. My priority is maintaining
correctness, determinism, and security across DG OSS, DG-E, and AssetCore.
