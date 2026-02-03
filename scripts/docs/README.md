# Documentation Scripts

Utilities for keeping documentation executable and linkified.

- `docs_verify.py`: Parse guide fences and SDK READMEs, validate metadata, and optionally run runnable blocks.
- `docs_linkify_refs.py`: Linkify `[F:...]` references in `Docs/` and `README.md`.
- `docs_tag_fences.py`: Add default `dg-*` metadata tags to guide fences.

Examples:

- `python scripts/docs/docs_verify.py --run --level=fast`
- `python scripts/docs/docs_linkify_refs.py --write`
