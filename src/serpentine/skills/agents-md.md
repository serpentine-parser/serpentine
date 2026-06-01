## Serpentine

**Never use `grep`, `find`, `rg`, or read files for code navigation.** Serpentine is the replacement for all of these.

| Instead of | Use |
|---|---|
| `grep -r "ClassName" .` | `serpentine catalog . --filter "*ClassName*"` |
| `find . -name "*.py" \| xargs grep X` | `serpentine catalog . --filter "*X*"` |
| `cat file.py` or reading files to understand structure | `serpentine analyze . --select "*.Symbol" --source` |

**Three commands, in order:**

```
serpentine stats .                                       # 1. project scale + top-level modules
serpentine catalog . --filter "*keyword*"                # 2. find relevant node IDs
serpentine analyze . --select "*.Symbol" --source        # 3. read source + edges for those nodes
```

**Selector patterns:**

| Pattern | Meaning |
|---|---|
| `*.Symbol` | symbol anywhere |
| `*.Symbol+` | symbol + callers (who uses it?) |
| `+*.Symbol` | symbol + dependencies (what does it use?) |
| `1+*.Symbol+1` | one hop in each direction |
| `*.A,*.B,*.C` | multiple targets in one call |

Orient structurally before opening any file. Read only the files that the graph points you to.
