// `use:tooltip={text}` - a styled tooltip after --delay-tooltip (400 ms) of hover.
// Hides on leave and on press; moving on to the next anchor shows it at once.
// `{ text, truncated: true }` shows it only while the text of the node is cut off (a long
// row title, one line or clamped to two): measured once when the pointer enters, never per
// frame.
// `{ text, hint }` adds a second, smaller line under the title, like the tooltips of native
// apps name a key: "Suchen" over "Strg+F". The hint rides on the anchor as
// `data-tooltip-hint`, where the tooltip layer (components/Tooltip.svelte) reads it.

import type { Action } from 'svelte/action';
import { tooltipDelay } from '../motion/motion';
import { tooltipState, type TooltipPlacement } from '../state/tooltip.svelte';

export type TooltipParam =
  | string
  | null
  | undefined
  | {
      text: string | null | undefined;
      placement?: TooltipPlacement;
      truncated?: boolean;
      /** A second, smaller line (a key, a hint). */
      hint?: string | null;
    };

interface Options {
  text: string;
  placement: TooltipPlacement;
  truncated: boolean;
  hint: string | null;
}

/** The attribute that carries the second line to the tooltip layer. */
export const TOOLTIP_HINT = 'tooltipHint';

function normalize(param: TooltipParam): Options | null {
  if (param === null || param === undefined) return null;
  if (typeof param === 'string') {
    return param === '' ? null : { text: param, placement: 'bottom', truncated: false, hint: null };
  }
  if (!param.text) return null;
  return {
    text: param.text,
    placement: param.placement ?? 'bottom',
    truncated: param.truncated ?? false,
    hint: param.hint || null,
  };
}

export const tooltip: Action<HTMLElement, TooltipParam> = (node, param) => {
  let options = normalize(param);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const mark = (): void => {
    if (options?.hint) node.dataset[TOOLTIP_HINT] = options.hint;
    else delete node.dataset[TOOLTIP_HINT];
  };
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
    // Cut off across (one line) or down (a clamped second line).
    const cut = node.scrollWidth > node.clientWidth || node.scrollHeight > node.clientHeight + 1;
    if (options.truncated && !cut) return;
    if (tooltipState.warm) show();
    else timer = setTimeout(show, tooltipDelay());
  };
  const leave = (): void => {
    cancel();
    tooltipState.hide(node);
  };

  mark();
  node.addEventListener('pointerenter', enter);
  node.addEventListener('pointerleave', leave);
  node.addEventListener('pointerdown', leave);

  return {
    update(next: TooltipParam) {
      options = normalize(next);
      mark();
      if (options === null) leave();
      else tooltipState.update(node, options.text);
    },
    destroy() {
      leave();
      delete node.dataset[TOOLTIP_HINT];
      node.removeEventListener('pointerenter', enter);
      node.removeEventListener('pointerleave', leave);
      node.removeEventListener('pointerdown', leave);
    },
  };
};
