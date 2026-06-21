from fastmcp import FastMCP


def register_prompts(mcp: FastMCP) -> None:
    @mcp.prompt()
    def build_selector(intent: str) -> str:
        """Guide the agent to build a selector for the given intent."""
        return f"""\
You want to select nodes from a Serpentine dependency graph to answer: "{intent}"

First, read the query guide for the full selector reference and decision tables:
  Resource: `serpentine://docs/query-guide`

Then follow this workflow:
1. Call `stats` tool with your repo_id and ref to understand graph scale.
2. Call `catalog` tool to discover node IDs — filter by keyword if you know one.
3. Call `analyze` with a selector built from those IDs.

Start by reading the query guide now.
"""

    @mcp.prompt()
    def analyze_repo(repo_id: str, ref: str, intent: str) -> str:
        """Full workflow: read the query guide, orient with stats and catalog, then analyze."""
        return f"""\
You are analyzing the repository `{repo_id}` at ref `{ref}`.

Goal: {intent}

**Step 0** — Read the query guide to understand selector syntax and workflow:
  Resource: `serpentine://docs/query-guide`

**Step 1** — Understand graph scale:
  Call tool: `stats` with repo_id="{repo_id}", ref="{ref}"

**Step 2** — Discover node IDs:
  Call tool: `catalog` with repo_id="{repo_id}", ref="{ref}"
  Filter the results to find nodes relevant to your goal.

**Step 3** — Query the graph:
  Call tool: `analyze` with repo_id="{repo_id}", ref="{ref}", and a selector
  built from the catalog IDs. Use source=true only after narrowing with a selector.

If you get a "not ingested" error at any step, call `ingest_ref` first:
  Call tool: `ingest_ref` with repo_id="{repo_id}", ref="{ref}"

Begin with Step 0.
"""
