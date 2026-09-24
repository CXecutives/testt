// `use:nearEnd={callback}` - calls back whenever the element (a sentinel at the end of a
// list) comes into view inside its scroll container. Progressive rendering of long lists.

import type { Action } from 'svelte/action';

export const nearEnd: Action<Element, () => void> = (node, callback) => {
  let current = callback;
  const observer = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) current();
  });
  observer.observe(node);
  return {
    update(next: () => void) {
      current = next;
    },
    destroy() {
      observer.disconnect();
    },
  };
};
