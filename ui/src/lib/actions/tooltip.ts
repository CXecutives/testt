// `use:tooltip={text}` - a styled tooltip after --delay-tooltip (350 ms) of hover.
// Hides on leave and on press; moving on to the next anchor shows it at once.

import type { Action } from 'svelte/action';
import { tooltipDelay } from '../motion/motion';
import { tooltipState, type TooltipPlacement } from '../state/tooltip.svelte';

export type TooltipParam =
  string | null | undefined | { text: string | null | undefined; placement?: TooltipPlacement };

function normalize(param: TooltipParam): { text: string; placement: TooltipPlacement } | null {
  if (param === null || param === undefined) return null;
  if (typeof param === 'string') return param === '' ? null : { text: param, placement: 'bottom' };
  if (!param.text) return null;
  return { text: param.text, placement: param.placement ?? 'bottom' };
}

export const tooltip: Action<HTMLElement, TooltipParam> = (node, param) => {
  let options = normalize(param);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const cancel = (): void => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const show = (): void => {
    timer = undefined;
    if (options !== null) tooltipState.show(node, options.text, options.placement);
  };
  const enter = (): void => {
    cancel();
    if (options === null) return;
    if (tooltipState.warm) show();
    else timer = setTimeout(show, tooltipDelay());
  };
  const leave = (): void => {
    cancel();
    tooltipState.hide(node);
  };

  node.addEventListener('pointerenter', enter);
  node.addEventListener('pointerleave', leave);
  node.addEventListener('pointerdown', leave);

  return {
    update(next: TooltipParam) {
      options = normalize(next);
      if (options === null) leave();
      else tooltipState.update(node, options.text);
    },
    destroy() {
      leave();
      node.removeEventListener('pointerenter', enter);
      node.removeEventListener('pointerleave', leave);
      node.removeEventListener('pointerdown', leave);
    },
  };
};
