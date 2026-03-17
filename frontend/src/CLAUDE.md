# Frontend Architecture — Agent Rules

This project uses a **domain-package architecture** with an **actor-based coordination layer** and an **explicit event bus**. Read this file before writing any code.

Deeper rules for specific areas live in subdirectory `CLAUDE.md` files — Claude Code loads them automatically when you work in those directories.

---

## Core Tenets

1. **Local by default.** Nothing is shared until it genuinely needs to be. Reuse requires a conscious, visible decision.
2. **Explicit coordination.** Every side effect that crosses domain boundaries is traceable to a single place (`app/`).
3. **Consistent segment structure.** Every domain uses the same internal layout: `model/`, `api/`, `lib/`, `config/`.

---

## Top-Level Layout

```
/src
  /domains         ← domain packages (ratings, critics, movies, similarity, …)
  /app
    actors.ts      ← actor instantiation and wiring (the ONLY place actors are created)
    bus.ts         ← event bus instance
    events.ts      ← domain event type contracts (zero imports — never add any)
    providers.tsx  ← makes actors available to the React tree
  /ui              ← shared design system, zero business logic, zero domain imports
  /pages           ← composes domains + ui, no business logic lives here
```

---

## Decision Guide — Work Through This Before Writing Code

### 1. Domain or app?

- **Business logic** (validation, computation, state transitions, data transformation) → belongs in a domain
- **Rendering, routing, layout** → belongs in `/pages` or `/ui`
- **Cross-domain coordination or external side effects as part of a workflow** → belongs in `app/`

### 2. Which segment?

| What you are writing                      | Where it goes                             |
| ----------------------------------------- | ----------------------------------------- |
| A TypeScript interface or type            | `model/types.ts`                          |
| A pure function or computation            | `model/`                                  |
| An XState machine definition              | `model/machine.ts`                        |
| A validation rule                         | `model/validate.ts`                       |
| A fetch function                          | `api/index.ts`                            |
| A mapper from API response to domain type | `api/mappers.ts`                          |
| A raw API response type                   | `api/types.ts`                            |
| A helper used only within this domain     | `lib/`                                    |
| A threshold, constant, or feature flag    | `config/`                                 |
| A React component                         | `/ui` or `/pages` — never inside a domain |

### 3. Does it need to be exported?

Default answer: **no.** Only add to `index.ts` what is genuinely consumed outside the domain. If unsure, keep it internal. Exporting is easy — un-exporting breaks consumers.

### 4. Does this coordination need XState?

Ask: does this action cross domain boundaries or trigger external side effects as part of a business workflow? If yes → model as an actor in `app/`. If self-contained within one domain or one component → use React Query or `useState`.

| Scenario                                                                          | Solution               |
| --------------------------------------------------------------------------------- | ---------------------- |
| User toggles a filter                                                             | `useState`             |
| User opens a modal                                                                | `useState`             |
| Server data fetching and caching                                                  | React Query            |
| User rates a movie → sync to server                                               | React Query mutation   |
| User rates a movie → server sync → similarity recompute → recommendations refresh | XState actor in `app/` |
| Multi-step workflow with error recovery and retry                                 | XState actor in `app/` |

### 5. Does this need a domain event?

Ask: does something in another domain need to react to this? If yes → add the event type to `app/events.ts` first, then publish from the relevant actor. Never put domain-specific types in the payload — only plain serializable data.

---

## Testing Requirements

| Layer                   | Approach                                         |
| ----------------------- | ------------------------------------------------ |
| `model/` pure functions | Unit tests, exhaustive input coverage            |
| `model/machine.ts`      | XState machine tests, all states and transitions |
| `api/`                  | Integration tests with msw (Mock Service Worker) |
| `app/` actor wiring     | Coordination tests simulating event sequences    |
| Critical user flows     | Playwright E2E                                   |

- `model/` is pure — test it with no React infrastructure
- `api/` tests cover the full stack from fetch through mapper to domain model shape, including error cases
- E2E tests cover flow correctness only — business logic is already covered in isolation
