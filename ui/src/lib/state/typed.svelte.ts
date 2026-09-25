// Text typed into a field that is no value yet (a chip field's draft before Enter or leaving
// the field). A form that shares a `TypedText` through the context counts it as a change, so
// saving, leaving and closing the window never lose it without asking.

import { getContext, setContext } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';

const KEY = Symbol('typed');

export class TypedText {
  /** Field id -> how to drop its text. */
  #fields = new SvelteMap<string, () => void>();

  /** Some field holds typed text that is no value yet. */
  get any(): boolean {
    return this.#fields.size > 0;
  }

  /** A field says whether it holds such text (`clear` drops it) or not (`null`). */
  set(id: string, clear: (() => void) | null): void {
    if (clear) this.#fields.set(id, clear);
    else this.#fields.delete(id);
  }

  /** Drops the typed text of every field (the form's changes are discarded). */
  clear(): void {
    for (const clear of [...this.#fields.values()]) clear();
    this.#fields.clear();
  }

  /** The fields inside the calling component report to this one. */
  share(): void {
    setContext(KEY, this);
  }
}

/** The `TypedText` of the form around the calling component, if it counts one. */
export function typedText(): TypedText | undefined {
  return getContext<TypedText | undefined>(KEY);
}
