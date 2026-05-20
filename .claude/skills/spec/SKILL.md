---
name: spec
description: Plan implementation before writing code. Analyzes codebase structure and dependencies, asks clarifying questions, and writes a scoped implementation spec.
argument-hint: <feature or change description>
allowed-tools: Bash Skill
---

Before writing any code for: $ARGUMENTS

**Step 1 — Structural analysis**

Run `/code-analysis` to get a codebase briefing — top-level modules, structure, and key files. Then identify which modules and functions this change will touch. For each one, run `/code-analysis <target>` to get its dependents, upstream dependencies, and source code.

Use the results to understand:

- Which files are relevant (from file paths and source blocks in the results)
- What the blast radius of this change is
- Where the architectural boundaries are that the spec should respect

Do not read files directly. Use the source blocks returned by the analysis instead.

**Step 2 — Clarifying questions**

Ask me about requirements, edge cases, and constraints. Use the structural analysis to ask informed questions — e.g. "the analysis shows X depends on Y, should the spec account for updating Y as well?"

**Step 3 — Write the spec**

Write a concise implementation plan to `.claude/scratch/spec-[feature-name].md` in the current directory. The spec should:

- Reference the code-analysis results (which modules are affected, what the blast radius is)
- For large features, break into smaller steps
- Note any BREAKING verdicts from the analysis and what callsites need updating
- Identify files to modify, in order

**Step 4 — Wait for approval**

Do not write any code until I approve the spec.
