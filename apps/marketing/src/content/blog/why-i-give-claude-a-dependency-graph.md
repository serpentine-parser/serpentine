---
title: "Files Are the Wrong Interface for AI Code Agents"
description: "When agents discover code file-by-file, they see syntax but miss relationships. A code reference graph makes call edges and blast radius explicit — so agents start from a map, not a pile."
pubDate: 2026-05-31
---

Every AI coding agent starts a task the same way: by reading files. Claude Code reaches for Read and Grep. Cursor scans the directory tree. Before any code gets written, the agent is orienting itself in your codebase — and that orientation is expensive. In a typical session, an agent reads a dozen or more files before generating a single line of output, spending most of its context on discovery rather than the actual work.

That's what you get when the only interface is a filesystem. Reading files to understand structure is the wrong abstraction for the job.

There's a better one: the code reference graph.

## The problem with file-by-file discovery

When an agent reads a file, it gets syntax: the literal text of your code. That's fine for narrow, self-contained tasks. But the moment a question crosses a module boundary, file-by-file discovery breaks down. The agent reads one file, infers it needs another, reads that, finds a reference to a third. Each hop burns tokens and adds latency. And even after all that reading, critical context still goes missing.

Files aren't the natural unit of a codebase. Relationships are — what calls what, what inherits from what, which modules depend on which. This structure is what an experienced engineer holds in their head, the thing that lets them answer "where would this change break?" in seconds. File reads surface syntax; they don't surface structure. You can read every file in a module and still not know what depends on it externally.

A code reference graph makes that structure explicit and queryable.

## Three commands to structural context

Serpentine gives you this graph in three CLI commands, and the output is designed to be dropped directly into an AI context window.

**Step 1: Orient.** Get the lay of the land — module names, rough scale, language breakdown.

```bash
serpentine stats .
```

This gives you node and edge counts, module hierarchy, and the languages detected. Takes under a second. Paste the output and Claude knows the shape of the project before looking at any code.

**Step 2: Locate.** Find the nodes relevant to the task.

```bash
serpentine catalog . --filter "*auth*"
```

This lists every node whose name contains "auth" — modules, classes, functions. You get exact node IDs, which you'll use in the next step.

**Step 3: Extract.** Pull the subgraph for the area you care about, with source.

```bash
serpentine analyze . --select "1+*.AuthService+1" --source
```

The `1+...+1` selector means: give me `AuthService`, one hop upstream (what it depends on), and one hop downstream (what depends on it). The `--source` flag includes the actual source code for each node. The output is a structured graph — node IDs, types, edges, positions, and code — that Claude can read like a map.

<!-- SCREENSHOT: UI_Overview.png — The Serpentine graph showing an interactive visualization of a module with its callers and callees expanded. Caption: "The same structural context, visualized in the browser." -->

## What the graph gives agents that file reads don't

The difference is edges. When an agent reads files, it sees names and syntax. When you give it a graph, it sees relationships:

- `AuthService` → calls → `TokenValidator`
- `AuthController` → calls → `AuthService`
- `AuthService` → is-a → `BaseService`

Those edges answer questions that syntax alone can't: "What breaks if I change this?" "Who else calls this?" "What does this class inherit from?" Claude can reason over a graph in a way it can't reason over a pile of files, because the relationships are explicit rather than implied.

The other advantage is density. A call graph for a 200-function module is typically a few kilobytes of structured text, far smaller than reading 200 functions worth of source file by file. You're giving the agent more semantic information per token, and cutting out the discovery loop entirely.

## The workflow in practice

Here's how I use this. I'm working on a feature that touches the graph state manager — the component that holds the in-memory representation of the analyzed project. Rather than dumping files:

```bash
# What is the state manager's full neighborhood?
serpentine analyze . --select "1+*.GraphStateManager+1" --source
```

I paste that output. Claude now knows: what `GraphStateManager` does, what it calls, what calls it, and where each piece lives in the filesystem. It can suggest changes without asking "where is X defined?" or "what's the signature of Y?"

When I ask Claude to add a new field to the state, it correctly identifies which callers will need to be updated. When I ask it to refactor a method, it knows the blast radius. I'm not babysitting the context — I loaded the right map upfront.

<!-- SCREENSHOT: Object_Explorer.png — The node detail panel showing a selected function with its edges (calls, called-by) and source code. Caption: "Selecting a node reveals its edges and source — the same information the CLI surfaces in structured text." -->

## Skills that use the graph automatically

The pattern above — orient, locate, extract — is useful enough that I've codified it into two skills in the project's `.claude/skills/` directory: `spec` and `debug`. Both are invoked from Claude Code with `/spec` and `/debug`, and both start the same way: run `/code-analysis` first.

`/spec` is for planning before writing code. Before a single line gets changed, it runs code-analysis on the modules the feature will touch — batched into one call using graph operators and comma-union selectors. It uses the blast radius output to ask informed clarifying questions. Claude already knows what the change depends on from the graph, so the questions are sharper: "the analysis shows `GraphStateManager` has three external callers; should the spec account for updating those, or are they out of scope?" The spec it produces references specific node IDs from the analysis and lists any BREAKING callsites explicitly.

`/debug` is for tracing unexpected behavior. Rather than asking you to paste files, it runs code-analysis on the suspected area and follows caller/callee edges to trace the call chain, without reading a single file directly. If the bug could be in multiple places, it batches all candidates into one call. The edges do the work: you can follow a chain from the symptom back to the source without knowing which files are involved upfront.

What both skills have in common: neither reads files directly. They rely entirely on the source blocks and edges returned by `code-analysis`. This keeps context lean: structure and relevant code, not every line in every touched file. The agent starts from a map rather than a pile.

The graph isn't just useful for orientation — it's the right interface for any task that requires understanding relationships before acting on them. Planning a feature requires knowing blast radius before you start, not after. Debugging requires tracing a call chain, not guessing which file to open. Building your skills around this pattern changes what the agent produces.

## Start with your own codebase

Install Serpentine, run `serpentine stats .` in your project root, and paste the output before asking your first question.

The orientation step alone cuts the back-and-forth. Combine it with targeted `analyze` calls scoped to the area you're working in, and the agent stops asking where things are and starts knowing.

```bash
pip install serpentine-parser
# or: cargo install serpentine
serpentine stats .
```

The graph is already in your codebase. Serpentine makes it readable.
