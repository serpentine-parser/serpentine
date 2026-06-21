from fastmcp import FastMCP


def register_prompts(mcp: FastMCP) -> None:
    @mcp.prompt()
    def build_selector(intent: str) -> str:
        """Explains selector syntax and guides the agent to build the right selector."""
        return f"""\
You want to select nodes from a Serpentine dependency graph to answer: "{intent}"

**Selector syntax** (dbt-style):
- `*.ClassName`       — a specific symbol by name, any module
- `+*.Symbol`         — symbol + all upstream dependencies (what it calls)
- `*.Symbol+`         — symbol + all downstream dependents (what calls it)
- `N+*.Symbol+M`      — bounded hops: N upstream, M downstream
- `@*.Symbol`         — full connected component
- `mod.sub.*`         — all nodes in a module
- `*.A,*.B`           — union of multiple patterns

**Recommended workflow:**
1. Read `serpentine://{{repo_id}}/{{ref}}/catalog` to discover node IDs.
2. Read `serpentine://{{repo_id}}/{{ref}}/stats` for graph scale.
3. Call the `analyze` tool with a selector built from the catalog IDs.

Start with the catalog resource now.
"""

    @mcp.prompt()
    def analyze_repo(repo_id: str, ref: str, intent: str) -> str:
        """Full workflow prompt: stats → catalog → selector → analyze."""
        return f"""\
You are analyzing the repository `{repo_id}` at ref `{ref}`.

Goal: {intent}

**Step 1** — Read stats to understand scale:
  Resource: `serpentine://{repo_id}/{ref}/stats`

**Step 2** — Read catalog to discover node IDs:
  Resource: `serpentine://{repo_id}/{ref}/catalog`

**Step 3** — Build a selector from catalog IDs, then call the `analyze` tool:
  Tool: `analyze` with `repo_id="{repo_id}"`, `ref="{ref}"`, and a selector expression.

If the ref has not been ingested yet, call the `ingest_ref` tool first:
  Tool: `ingest_ref` with `repo_id="{repo_id}"`, `ref="{ref}"`

Begin with Step 1.
"""
