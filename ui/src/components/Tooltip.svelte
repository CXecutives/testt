<!--
  The tooltip layer: mounted once (App, Gallery), fed by the `tooltip` action. Placed below
  its anchor (above if there is no room), kept inside the window, whole-pixel positions.
  The deep navy bubble pops toward its anchor (100 ms) and leaves with a 60 ms fade; moving
  on to the next anchor while it shows just moves it (no second entrance).
-->
<script lang="ts">
  import { px, setVars } from '$lib/actions/cssVars';
  import { tooltipIn, tooltipOut } from '$lib/motion/transitions';
  import { TOOLTIP_ID, tooltipState } from '$lib/state/tooltip.svelte';
  import { tokenPx } from '$lib/tokens';
  import type { Action } from 'svelte/action';

  const place: Action<HTMLElement, { anchor: HTMLElement; text: string }> = (node, params) => {
    const update = ({ anchor }: { anchor: HTMLElement; text: string }): void => {
      const gap = tokenPx('--tooltip-gap');
      const edge = tokenPx('--viewport-gap');
      const a = anchor.getBoundingClientRect();
      const width = node.offsetWidth;
      const height = node.offsetHeight;
      const maxX = document.documentElement.clientWidth - width - edge;
      const x = Math.max(edge, Math.min(a.left + a.width / 2 - width / 2, maxX));
      const below = a.bottom + gap;
      const above = a.top - gap - height;
      const fitsBelow = below + height <= document.documentElement.clientHeight - edge;
      const top =
        tooltipState.placement === 'top'
          ? above >= edge
            ? above
            : below
          : fitsBelow
            ? below
            : above;
      setVars(node, { 'tooltip-x': px(x), 'tooltip-y': px(top) });
    };
    update(params);
    return { update };
  };
</script>

{#if tooltipState.anchor}
  <div
    class="layer"
    role="tooltip"
    id={TOOLTIP_ID}
    use:place={{ anchor: tooltipState.anchor, text: tooltipState.text }}
  >
    <div class="bubble" in:tooltipIn={{ placement: tooltipState.placement }} out:tooltipOut>
      {tooltipState.text}
    </div>
  </div>
{/if}

<style>
  .layer {
    position: fixed;
    z-index: var(--z-tooltip);
    top: 0;
    left: 0;
    max-width: var(--tooltip-max);
    pointer-events: none;
    transform: translate(var(--tooltip-x), var(--tooltip-y));
  }

  /* Small and quiet like a native tooltip: 12 px text, 4/8 padding, a light shadow. */
  .bubble {
    padding: var(--space-4) var(--space-8);
    border-radius: var(--radius-xs);
    background-color: var(--surface-inverse);
    box-shadow: var(--sh-tooltip);
    color: var(--text-inverse);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    letter-spacing: var(--tracking-tooltip);
    overflow-wrap: anywhere;
  }
</style>
