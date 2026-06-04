---
name: gen-pr
description: Generate a PR description as a Markdown file in .claude/scratch using git history and code analysis. Fills in the project's PR template.
argument-hint: [optional: brief hint about what this PR does]
allowed-tools: Bash Skill Write
---

Generate a PR description for the current branch. $ARGUMENTS

**Step 1 — Gather git context (read-only)**

Run these git commands in parallel to understand what changed:

- `git log main..HEAD --oneline` — list commits on this branch
- `git diff main..HEAD --stat` — files changed and line counts
- `git diff main..HEAD -- '*.rs' '*.py' '*.ts' '*.tsx'` — full diff for source files (skip lock files, build artifacts)

Do NOT run any git command that writes, stages, or modifies state.

**Step 2 — Analyze changed symbols**

From the diff, identify the key symbols that were added or modified (functions, classes, types, components). Run a single `/code-analysis` call with the most important ones (comma-separated) to understand:

- What each changed symbol does and who calls it
- Whether the change is additive, modifying existing behavior, or a breaking change
- Which layers (Rust, Python, frontend) are touched

Do not read files directly. Use source blocks from the analysis.

**Step 3 — Write the PR description**

Write the completed PR description to `.claude/scratch/pr-[branch-name].md` in the project root, filling in the template below. Use only what you know — do not invent testing steps or checklist items you can't verify from the diff and analysis.

Template to fill in:

```
## Summary

[1–3 sentences: what this PR does and why. Be specific — name the symbols, modules, or behaviors changed.]

## Changes

- [one bullet per logical change; reference file paths or symbol names where useful]
- ...

## Testing

[Describe what was actually changed and how a reviewer would verify it. Name specific commands, flags, or behaviors to check. If the change is Rust-only CLI/lib work, note that; if it touches the frontend, mention what to look at in the browser.]

## Checklist

- [ ] Rust changes: `cargo clippy` passes
- [ ] TypeScript changes: `npx tsc --noEmit` passes in `frontend/`
- [ ] Python changes: type annotations added to any new functions
- [ ] Tested end-to-end with `serpentine serve`
```

Only check a checklist item if the diff confirms no changes in that area (e.g. no `.rs` files changed → Rust item is not relevant, omit it) or if you can verify compliance from the diff itself. Leave items unchecked when in doubt.

**Step 4 — Report**

Tell me the path to the generated file and give a one-paragraph summary of what you wrote so I can decide whether to use it as-is or ask for edits.
