# System Test Scripts

Registry-driven system-test tooling.

- `test_runner.py`: Runs system tests by registry category or priority.
- `coverage_report.py`: Generates coverage and infrastructure docs in `Docs/testing/`.
- `gap_tracker.py`: Lists, closes, and generates prompts for coverage gaps.

Examples:

- `python scripts/system_tests/test_runner.py --priority P0`
- `python scripts/system_tests/coverage_report.py generate`
- `python scripts/system_tests/gap_tracker.py list`
