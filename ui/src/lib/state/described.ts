// The text that describes the control inside a wrapper (the hint or error of a Field, the
// hint of a SettingRow). The wrapper knows whether it shows that text, so it shares the
// text's id only while it does: a control's `aria-describedby` never names a missing id.

import { getContext, setContext } from 'svelte';

const KEY = Symbol('described');

/** The calling wrapper describes the controls inside it with the element `id` returns
 *  (`null` while it shows none). */
export function describe(id: () => string | null): void {
  setContext(KEY, id);
}

/** The id of the text the wrapper around the calling control shows, or `null`. */
export function describedBy(): () => string | null {
  return getContext<(() => string | null) | undefined>(KEY) ?? (() => null);
}
