// `use:inView={callback}` - tells where a sentinel element is relative to its scroll area:
// 'in' while it can be seen, 'above' once it has scrolled away over the top edge, 'below'
// while it has not been reached. Scroll-driven UI (the list header's hairline, the reader's
// compact bar) hangs on it: an IntersectionObserver on the nearest scroll area, no scroll
// listener and no layout reads while scrolling.

import type { Action } from 'svelte/action';

export type Place = 'in' | 'above' | 'below';

/** The nearest ancestor that scrolls vertically (the viewport if there is none). */
export function scrollArea(node: Element): Element | null {
  for (let parent = node.parentElement; parent !== null; parent = parent.parentElement) {
    if (/auto|scroll/.test(getComputedStyle(parent).overflowY)) return parent;
  }
  return null;
}

export const inView: Action<Element, (place: Place) => void> = (node, callback) => {
  let current = callback;
  const observer = new IntersectionObserver(
    (entries) => {
      const entry = entries.at(-1);
      if (entry === undefined) return;
      if (entry.isIntersecting) {
        current('in');
        return;
      }
      const top = entry.rootBounds?.top ?? 0;
      current(entry.boundingClientRect.bottom <= top ? 'above' : 'below');
    },
    { root: scrollArea(node) },
  );
  observer.observe(node);
  return {
    update(next: (place: Place) => void) {
      current = next;
    },
    destroy() {
      observer.disconnect();
    },
  };
};
