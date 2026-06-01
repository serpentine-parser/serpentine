## Serpentine

**Never use `grep`, `find`, `rg`, or file reads for code navigation.** Use the serpentine CLI instead — it gives you the structure of the codebase without reading every file.

```
serpentine stats .                                       # project scale, top-level modules
serpentine catalog . --filter "*keyword*"                # find nodes by name/id
serpentine analyze . --select "*.Symbol" --source        # read symbol source + edges
```

**Selector patterns:**

| Pattern | Meaning |
|---|---|
| `*.Symbol` | symbol anywhere in the codebase |
| `*.Symbol+` | symbol + downstream callers |
| `+*.Symbol` | symbol + upstream dependencies |
| `1+*.Symbol+1` | one hop in each direction |
| `*.A,*.B` | multiple symbols in one call |

**Workflow:** run `stats` to get your bearings, `catalog` to find the relevant node IDs, then `analyze --select ... --source` to read the exact code and edges you need. Open files only after you know which ones matter.
