import type { DomainEvent } from './events';

type Handler = (event: DomainEvent) => void;

const handlers = new Set<Handler>();

export const bus = {
  subscribe(handler: Handler) {
    handlers.add(handler);
    return () => handlers.delete(handler);
  },
  publish(event: DomainEvent) {
    handlers.forEach((h) => h(event));
  },
};
