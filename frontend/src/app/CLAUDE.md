# App Layer Rules

These rules apply to everything inside `/src/app/`. This is the **coordination layer** — the only place that knows the full picture of how domains interact.

---

## What Lives Here

| File            | Purpose                                                            |
| --------------- | ------------------------------------------------------------------ |
| `actors.ts`     | The **only** place actors are instantiated and wired together      |
| `bus.ts`        | The event bus instance                                             |
| `events.ts`     | Domain event type contracts — zero imports, pure type declarations |
| `providers.tsx` | Makes actors available to the React tree                           |

---

## XState Actor Rules

### Machines vs. Actors

- **Machine definitions** (the `setup({}).createMachine({…})` call) live in `domains/<name>/model/machine.ts`
- **Actor instances** (`createActor(machine)`) live **only** in `app/actors.ts` — never anywhere else

```typescript
// ✅ Correct: instantiate in app/actors.ts
import { ratingMachine } from "@domains/ratings";
export const ratingActor = createActor(ratingMachine);

// ❌ Wrong: never instantiate inside a domain or component
const actor = createActor(ratingMachine); // not here
```

### Actor Communication

- Actors communicate with each other **via the event bus**, not by importing each other directly
- An actor must never directly import from another domain — it publishes to the bus and lets subscribers react
- React Query logic (fetch, mutate) does not live inside a machine definition — machines invoke service functions; service functions call React Query

### React Query and XState Responsibilities

| Concern                                          | Owner                                                |
| ------------------------------------------------ | ---------------------------------------------------- |
| Fetching and caching server data                 | React Query                                          |
| Deciding _when_ to fetch                         | XState actor                                         |
| Handling fetch success/failure in a workflow     | XState actor                                         |
| Invalidating cache after a mutation              | XState actor (calls `queryClient.invalidateQueries`) |
| Optimistic UI updates                            | React Query mutation options                         |
| Business workflow state (saving, error, syncing) | XState actor                                         |
| Simple local UI state (open, selected, hovered)  | `useState`                                           |

> React Query is an adapter. XState is the orchestrator. React Query does not know XState exists.

### Prefer `actor.subscribe` over `useEffect`

- When reacting to actor state changes in React, use `actor.subscribe` — not `useEffect` watching derived state

---

## Event Bus Rules

### `app/events.ts` — The Contract File

- **Zero imports. Always.** This file may never import from any domain, any library, or any other file.
- It contains only TypeScript type declarations
- Changing it is a deliberate, reviewable, consequential act — TypeScript will surface every broken consumer

### Event Payload Rules

- All date fields are **ISO strings** — never `Date` objects
- All IDs are **strings**
- Payloads contain only **primitives and plain objects** — no class instances, no domain-specific types
- Event `type` names are `SCREAMING_SNAKE_CASE`
- Names describe **what happened**, not what to do: `RATING_CHANGED` ✅ not `UPDATE_SIMILARITY` ❌
- Use **past tense**: `RATING_CHANGED`, `CRITIC_FETCHED`, `SCORES_UPDATED`

### Subscription Rules

- Domains **never** subscribe to events directly — all subscriptions belong in `app/`
- A bus subscription handler should only call an actor or service function — business logic does not live inside a handler

---

## Red Flags in This Layer

- An actor instantiated anywhere other than `app/actors.ts`
- An actor that imports directly from another domain instead of using the bus
- An event payload containing a `Date` object instead of an ISO string
- An event payload that imports a type from a domain package
- An event name in imperative form (`RATE_MOVIE`) instead of past tense (`RATING_CHANGED`)
- A domain subscribing to bus events — subscriptions belong here, in `app/`
- Business logic inside a subscription handler — handlers only delegate to actors or services
- React Query fetch/mutate logic inside a machine definition
