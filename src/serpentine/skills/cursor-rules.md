---
description: Use serpentine for structural code orientation before reading files
alwaysApply: true
---

# Serpentine

Before reading files or searching with grep/find, use the serpentine CLI to orient structurally:

```
serpentine stats .                                      # project scale + top-level modules
serpentine catalog . --filter "*auth*"                  # find relevant nodes by name
serpentine analyze . --select "*.AuthService" --source  # read symbol source + edges
```

**Selector patterns:**

| Pattern | Meaning |
|---|---|
| `*.Symbol` | symbol anywhere |
| `*.Symbol+` | symbol + callers |
| `+*.Symbol` | symbol + dependencies |
| `1+*.Symbol+1` | one hop each direction |

**Never use grep/find/rg for code navigation.** Run serpentine first to find the right nodes, then open only the files that matter.
