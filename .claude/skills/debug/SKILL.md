---
name: debug
description: Diagnose and fix a bug or unexpected behavior. Traces the problem through the dependency graph, adds logging if needed, and proposes a fix. Use when something is broken, producing wrong output, or behaving unexpectedly.
argument-hint: <description of the bug or unexpected behavior>
allowed-tools: Bash Edit Skill
---

Diagnose and fix: $ARGUMENTS

**Step 1 — Understand the problem**

Ask me what the intended behavior is, what the current output is, and how to reproduce it. Do not start investigating until you understand what "fixed" looks like.

**Step 2 — Trace through the graph**

Run `/code-analysis <suspected function or module>` on the area where the bug likely lives. Use the edges to trace the call chain and the source blocks to read the code — follow caller/callee relationships to find where the bad data or wrong behavior originates.

If the bug could be in multiple places, run analysis on each candidate. If you don't know where to start, run `/code-analysis` with no arguments first to get the module boundaries.

Do not read files directly. Use the source blocks returned by the analysis instead.

**Step 3 — Narrow down**

Don't overthink. If the graph narrows it to a few files but you're unsure which is the source, add logging statements to gain a better understanding. Run the code and read the output.

**Step 4 — Diagnose**

Write a brief diagnosis: what's broken, why, and which callsites are affected (from the code-analysis results). Share it with me before proposing a fix.

**Step 5 — Wait for approval**

Propose the fix. Do not write any code until I approve. If the fix touches a function with a BREAKING verdict from check, list all affected callsites in the proposal.
