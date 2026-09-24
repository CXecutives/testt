// CSS transitions that follow a runtime value (the segmented thumb, the nav pill, a meter)
// must not play when their component mounts or a view comes back: the value arrives through
// an action after the element exists, and a style computed in between would make the
// first placement slide. `settled()` turns true only after the first frame has been drawn
// with the final values; components switch those transitions on with it.

/** Call in a component's script: `const motion = settled();` then `class:ready={motion.ready}`. */
export function settled(): { readonly ready: boolean } {
  let ready = $state(false);
  $effect(() => {
    let frame = requestAnimationFrame(() => {
      // One frame drawn at rest, the transitions come on in the next.
      frame = requestAnimationFrame(() => (ready = true));
    });
    return () => cancelAnimationFrame(frame);
  });
  return {
    get ready() {
      return ready;
    },
  };
}
