// Typed access to single run event kinds, built on the one run channel of api.ts.

import { onRun } from './api';
import type { RunEvent } from './types';

export type RunEventType = RunEvent['type'];
export type RunEventOf<T extends RunEventType> = Extract<RunEvent, { type: T }>;

export function isRunEvent<T extends RunEventType>(
  event: RunEvent,
  type: T,
): event is RunEventOf<T> {
  return event.type === type;
}

/** Handle one kind of run event. Returns an unsubscribe function. */
export function onRunEvent<T extends RunEventType>(
  type: T,
  handler: (event: RunEventOf<T>) => void,
): () => void {
  return onRun((event) => {
    if (isRunEvent(event, type)) handler(event);
  });
}
