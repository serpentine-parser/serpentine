---
name: bump-version
description: Bump the project version across all files that carry it. Pass the new version as the argument (e.g. /bump-version 0.4.0).
argument-hint: <new version>
allowed-tools: Read Edit Bash
---

Bump the project version to $ARGUMENTS across every file listed below. For each file, search for the current version string and replace it with $ARGUMENTS.

## Files to update

| File | What to change |
|------|----------------|
| `pyproject.toml` | `version = "..."` under `[project]` |
| `rust/Cargo.toml` | `version = "..."` under `[package]` |
| `src/serpentine/__init__.py` | `__version__ = "..."` |
| `frontend/package.json` | `"version": "..."` |
| `apps/marketing/src/pages/index.astro` | badge text `v... · Apache 2.0` and CTA badge `{ dot: false, label: 'v...' }` |

## Steps

1. Run `grep -r "OLD_VERSION" . --include="*" -l | grep -v .git` with the current version to confirm no new files have been added since this skill was written. If any unexpected files appear, update them too and add them to this skill's table.

2. Read each file listed above, then apply the edits.

3. After all edits, tell the user to regenerate the lock files:
   ```
   uv lock                             # regenerates uv.lock
   cd rust && cargo update             # regenerates rust/Cargo.lock (bare cargo build will fail — PyO3 needs maturin)
   cd frontend && npm install          # regenerates frontend/package-lock.json
   ```

## Notes

- **Do not manually edit lock files** (`uv.lock`, `rust/Cargo.lock`, `frontend/package-lock.json`). They are auto-generated and contain checksums — manual edits leave them inconsistent. Let the build tools regenerate them.
- `apps/marketing/bun.lock` contains `"db0": ">=0.2.1"` — this is a third-party peer dependency constraint, not the project version. Do not change it.
- `index.astro` has two version references: the hero badge and the CTA badge at the bottom. Update both.
