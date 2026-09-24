// `use:reveal={callback}` - calls back once, the first time the element is at least half
// visible (score rings and stat tiles count up then, not while off screen).

import type { Action } from 'svelte/action';

export const reveal: Action<Element, () => void> = (node, callback) => {
  let done = false;
  let current = callback;
  const observer = new IntersectionObserver(
    (entries) => {
      if (done || !entries.some((entry) => entry.isIntersecting)) return;
      done = true;
      observer.disconnect();
      current();
    },
    { threshold: 0.5 },
  );
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
