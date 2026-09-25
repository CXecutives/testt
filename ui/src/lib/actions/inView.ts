// `use:inView={callback}` - tells where a sentinel element is relative to its scroll area:
// 'in' while it can be seen, 'above' once it has scrolled away over the top edge, 'below'
// while it has not been reached. Scroll-driven UI (the list header's hairline, the reader's
// compact bar) hangs on it: an IntersectionObserver on the nearest scroll area, no scroll
// listener and no layout reads while scrolling.
//
// The scroll area is looked up once the frame that shows the sentinel has been drawn: its
// styles are read then, when they are ready. Read while the new content is still unstyled
// (a job that just opened in the reader), they would force the whole page to be styled and
// laid out in the middle of the script. Until then the sentinel counts as 'in', the state
// every user starts from, and an observer reports its first answer after a frame anyway.

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
  let observer: IntersectionObserver | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const watch = (): void => {
    observer = new IntersectionObserver(
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
  };
  const frame = requestAnimationFrame(() => {
    timer = setTimeout(watch, 0);
  });

  return {
    update(next: (place: Place) => void) {
      current = next;
    },
    destroy() {
      cancelAnimationFrame(frame);
      clearTimeout(timer);
      observer?.disconnect();
    },
  };
};
