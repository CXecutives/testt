// The one tooltip of the app. The `tooltip` action writes here; the Tooltip component
// renders it. A native `title` is forbidden (unstyled, slow, sticks to the OS look).

/** Below, above, or right of the anchor (the icons of a vertical rail, like a native
 *  sidebar: a bubble below would cover the next entry). */
export type TooltipPlacement = 'bottom' | 'top' | 'right';

export const TOOLTIP_ID = 'app-tooltip';

/** Moving from one tooltip to the next within this time shows the next one at once. */
const WARM_MS = 400;

class TooltipState {
  anchor = $state<HTMLElement | null>(null);
  text = $state('');
  placement = $state<TooltipPlacement>('bottom');
  #hiddenAt = 0;

  get warm(): boolean {
    return this.anchor !== null || performance.now() - this.#hiddenAt < WARM_MS;
  }

  show(anchor: HTMLElement, text: string, placement: TooltipPlacement): void {
    this.anchor = anchor;
    this.text = text;
    this.placement = placement;
    anchor.setAttribute('aria-describedby', TOOLTIP_ID);
  }

  update(anchor: HTMLElement, text: string): void {
    if (this.anchor === anchor) this.text = text;
  }

  hide(anchor?: HTMLElement): void {
    if (this.anchor === null || (anchor !== undefined && anchor !== this.anchor)) return;
    this.anchor.removeAttribute('aria-describedby');
    this.anchor = null;
    this.#hiddenAt = performance.now();
  }
}

export const tooltipState = new TooltipState();
