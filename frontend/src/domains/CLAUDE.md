# Domain Rules

These rules apply to everything inside `/src/domains/`. They are loaded automatically when you work in any domain.

---

## Domain Segment Structure

Every domain uses exactly these four segments. Their meaning is fixed — do not invent new ones.

| Segment   | Responsibility                                                                      |
| --------- | ----------------------------------------------------------------------------------- |
| `model/`  | Business logic: types, pure functions, XState machine definitions, validation rules |
| `api/`    | External communication: fetch functions, request/response types, data mappers       |
| `lib/`    | Utilities that support **this domain only** — not shared with other domains         |
| `config/` | Domain-specific configuration: thresholds, API endpoints, feature flags, constants  |

Not every domain needs every segment. The absence of a segment is intentional and informative — do not add one unless there is genuine need.

---

## The index.ts Contract

Every domain exposes a public interface through a single `index.ts` at its root.

**Never import from a domain's internal paths.** If it is not exported from `index.ts`, it does not exist to the outside world.

```typescript
// domains/ratings/index.ts — only exports are part of the public API
export { ratingMachine } from "./model/machine";
export { validateRating } from "./model/validate";
export type { Rating, RatingInput } from "./model/types";

// api/client.ts, lib/normalize.ts, etc. are NOT exported — they are implementation details
```

The friction of adding an export is intentional. Reuse requires a conscious decision.

---

## Boundary Rules — Never Violate These

- **Never import from another domain's internal files.** Only import from its `index.ts`.
  - ❌ `import { normalize } from '@domains/ratings/lib/normalize'`
  - ✅ `import { validateRating } from '@domains/ratings'`
- **Never import from `app/` inside a domain.** Dependency flows downward only: `app/` depends on domains, never the reverse. The only exception is importing event types from `app/events.ts`.
- **Never import from `/pages` inside a domain.**
- **`app/events.ts` must always have zero imports.** Never add an import to that file for any reason.

---

## Segment Rules — Never Violate These

### `model/` is pure — it never has side effects

- No fetch calls, no API calls, no HTTP requests of any kind
- No `useEffect`, no React hooks
- No actor instantiation (machine _definitions_ live here; actor _instances_ live in `app/actors.ts`)
- Pure functions only: given the same input, always return the same output

### `api/` transforms shape — it does not enforce rules

- Mappers transform API response shape to domain model shape
- Business logic and validation rules do **not** belong in `api/` — put them in `model/`
- Raw API response types belong in `api/types.ts`, not in `model/types.ts`

### `lib/` is domain-private

- If a helper is clearly reusable across domains, it does not belong in `lib/` — reconsider `/ui/lib` or a shared utility
- Never import from one domain's `lib/` in another domain

### `config/` is static

- Constants, thresholds, feature flags, API endpoint strings
- No logic, no functions that compute values at runtime

---

## General Red Flags

- A component inside a domain — React components belong in `/ui` or `/pages`, never in a domain
- A `model/` file longer than ~150 lines — likely a signal the domain should be split
- A `useEffect` in domain code that coordinates between two pieces of state — this is usually an actor in disguise; move it to `app/`
- Business logic or validation inside `api/mappers.ts` — mappers only reshape data
