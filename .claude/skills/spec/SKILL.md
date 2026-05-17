---
name: spec
description: Plan implementation before writing code. Analyzes codebase structure and dependencies, asks clarifying questions, and writes a scoped implementation spec.
argument-hint: <feature or change description>
allowed-tools: Bash Read Skill
---

Before writing any code for: $ARGUMENTS

**Step 1 — Structural analysis**

If you have not run `/serpentine-orient` this session, run it now to get the codebase briefing.

Then identify which modules and functions this change will touch. For each one, run `/serpentine-check <target>` to get its dependents and upstream dependencies. If the change spans multiple areas, run check on each.

Use the orient briefing and check verdicts to understand:

- Which files are relevant (from the node IDs and file paths in the results)
- What the blast radius of this change is
- Where the architectural boundaries are that the spec should respect

Do not read files manually until you have the serpentine results. Only read files that the analysis identifies as relevant.

**Step 2 — Clarifying questions**

Ask me about requirements, edge cases, and constraints. Use the structural analysis to ask informed questions — e.g. "serpentine shows X depends on Y, should the spec account for updating Y as well?"

**Step 3 — Write the spec**

Write a concise implementation plan to `.claude/scratch/spec-$0.md` in the current directory. The spec should:

- Reference the serpentine analysis (which modules are affected, what the blast radius is)
- For large features, break into smaller steps
- Note any BREAKING verdicts from check and what callsites need updating
- Identify files to modify, in order

**Step 4 — Wait for approval**

Do not write any code until I approve the spec.
