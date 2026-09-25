// `use:presence={(here) => ...}` - says when the pointer comes onto the element or the focus
// into it (`here` true), and when both have left it again (false). Moves between the
// element's own children do not count. A list row builds its tools only while it is here.

import type { Action } from 'svelte/action';

export const presence: Action<HTMLElement, (here: boolean) => void> = (node, callback) => {
  let current = callback;
  let pointer = false;
  let focus = false;
  let here = false;

  const update = (): void => {
    const next = pointer || focus;
    if (next === here) return;
    here = next;
    current(next);
  };
  const enter = (): void => {
    pointer = true;
    update();
  };
  const leave = (): void => {
    pointer = false;
    update();
  };
  const focusIn = (): void => {
    focus = true;
    update();
  };
  const focusOut = (event: FocusEvent): void => {
    const next = event.relatedTarget;
    if (next instanceof Node && node.contains(next)) return;
    focus = false;
    update();
  };

  node.addEventListener('pointerenter', enter);
  node.addEventListener('pointerleave', leave);
  node.addEventListener('focusin', focusIn);
  node.addEventListener('focusout', focusOut);
  return {
    update(next: (here: boolean) => void) {
      current = next;
    },
    destroy() {
      node.removeEventListener('pointerenter', enter);
      node.removeEventListener('pointerleave', leave);
      node.removeEventListener('focusin', focusIn);
      node.removeEventListener('focusout', focusOut);
    },
  };
};
