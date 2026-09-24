<!--
  The button of the app: primary | secondary | ghost | danger × sm | md | lg. Flat like a
  native button: hover and press change colour only (100 ms), no lift, no glow, no bounce.
  - Trailing actions inside a row are sm, action bars are md.
  - At most one primary per view (checked by core/tests/ui_contract.rs).
  - iconOnly needs its label: it becomes aria-label and tooltip.
  - Disabled buttons stay hoverable (aria-disabled) so the tooltip can say why.
  - Loading keeps the width: the content stays in place, invisible, under the spinner.
-->
<script lang="ts" module>
  export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
  export type ButtonSize = 'sm' | 'md' | 'lg';
  export const BUTTON_VARIANTS: readonly ButtonVariant[] = [
    'primary',
    'secondary',
    'ghost',
    'danger',
  ];
  export const BUTTON_SIZES: readonly ButtonSize[] = ['sm', 'md', 'lg'];
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import Icon, { type IconName, type IconSize } from './Icon.svelte';
  import Spinner from './Spinner.svelte';

  interface Props {
    label: string;
    variant?: ButtonVariant;
    size?: ButtonSize;
    icon?: IconName | null;
    iconOnly?: boolean;
    loading?: boolean;
    disabled?: boolean;
    /** Why the button is disabled - shown in the tooltip. */
    disabledReason?: string | null;
    type?: 'button' | 'submit';
    /** Toggle buttons (e.g. the pin star). */
    pressed?: boolean | null;
    /** Fill the width of the container. */
    wide?: boolean;
    testid?: string | null;
    onclick?: (event: MouseEvent) => void;
  }

  let {
    label,
    variant = 'secondary',
    size = 'md',
    icon = null,
    iconOnly = false,
    loading = false,
    disabled = false,
    disabledReason = null,
    type = 'button',
    pressed = null,
    wide = false,
    testid = null,
    onclick,
  }: Props = $props();

  const ICON_SIZE: Record<ButtonSize, IconSize> = { sm: 'sm', md: 'sm', lg: 'md' };
  const ICON_ONLY_SIZE: Record<ButtonSize, IconSize> = { sm: 'sm', md: 'md', lg: 'lg' };

  const inactive = $derived(disabled || loading);
  const hint = $derived(disabled && disabledReason ? disabledReason : iconOnly ? label : null);

  function handle(event: MouseEvent): void {
    if (inactive) {
      event.preventDefault();
      return;
    }
    onclick?.(event);
  }
</script>

<button
  {type}
  class="btn {variant} {size}"
  class:icon-only={iconOnly}
  class:wide={wide && !iconOnly}
  class:loading
  aria-label={iconOnly ? label : undefined}
  aria-disabled={disabled ? 'true' : undefined}
  aria-busy={loading ? 'true' : undefined}
  aria-pressed={pressed === null ? undefined : pressed}
  data-testid={testid ?? undefined}
  use:tooltip={hint}
  onclick={handle}
>
  <span class="content">
    {#if icon}
      <Icon
        name={icon}
        size={iconOnly ? ICON_ONLY_SIZE[size] : ICON_SIZE[size]}
        filled={pressed === true}
      />
    {/if}
    {#if !iconOnly}
      <span class="label">{label}</span>
    {/if}
  </span>
  {#if loading}
    <span class="busy"><Spinner size="sm" label={null} /></span>
  {/if}
</button>

<style>
  .btn {
    position: relative;
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    height: var(--btn-height);
    padding: 0 var(--btn-pad);
    border: var(--border-width) solid var(--btn-border);
    border-radius: var(--radius-control);
    background-color: var(--btn-bg);
    box-shadow: var(--btn-shadow);
    color: var(--btn-fg);
    font: var(--btn-type);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .content {
    display: inline-flex;
    align-items: center;
    gap: var(--btn-gap);
  }

  .busy {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .loading .content {
    opacity: 0;
  }

  .btn:not([aria-disabled='true'], .loading):hover {
    border-color: var(--btn-border-hover);
    background-color: var(--btn-bg-hover);
    color: var(--btn-fg-hover);
  }

  /* Pressed: only while the left button is down (input.ts keeps the others from pressing). */
  .btn:not([aria-disabled='true'], .loading):active {
    border-color: var(--btn-border-active);
    background-color: var(--btn-bg-active);
    transition-duration: var(--dur-instant);
  }

  .btn:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .btn[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .btn[aria-disabled='true'],
  .loading {
    cursor: default;
  }

  /* ------------------------------------------------------------ variants */
  .primary {
    --btn-bg: var(--primary);
    --btn-bg-hover: var(--primary-hover);
    --btn-bg-active: var(--primary-active);
    --btn-border: var(--primary);
    --btn-border-hover: var(--primary-hover);
    --btn-border-active: var(--primary-active);
    --btn-fg: var(--text-on-accent);
    --btn-fg-hover: var(--text-on-accent);
    --btn-shadow: var(--sh-xs);
  }

  .secondary {
    --btn-bg: var(--surface);
    --btn-bg-hover: var(--surface-muted);
    --btn-bg-active: var(--surface-muted);
    --btn-border: var(--border-strong);
    --btn-border-hover: var(--border-input);
    --btn-border-active: var(--border-input);
    --btn-fg: var(--text);
    --btn-fg-hover: var(--text);
    --btn-shadow: var(--sh-xs);
  }

  /* A secondary toggle that is on (a filter chip): the ink edge on a muted surface. */
  .secondary[aria-pressed='true'] {
    --btn-bg: var(--surface-muted);
    --btn-border: var(--text);
    --btn-border-hover: var(--text);
  }

  .ghost {
    --btn-bg: transparent;
    --btn-bg-hover: var(--surface-hover);
    --btn-bg-active: var(--surface-press);
    --btn-border: transparent;
    --btn-border-hover: transparent;
    --btn-border-active: transparent;
    --btn-fg: var(--text-muted);
    --btn-fg-hover: var(--text);
    --btn-shadow: none;
  }

  .ghost[aria-pressed='true'] {
    --btn-fg: var(--pressed);
    --btn-fg-hover: var(--pressed);
  }

  .danger {
    --btn-bg: var(--danger-strong);
    --btn-bg-hover: var(--danger-hover);
    --btn-bg-active: var(--danger-active);
    --btn-border: var(--danger-strong);
    --btn-border-hover: var(--danger-hover);
    --btn-border-active: var(--danger-active);
    --btn-fg: var(--text-on-accent);
    --btn-fg-hover: var(--text-on-accent);
    --btn-shadow: var(--sh-xs);
  }

  /* --------------------------------------------------------------- sizes */
  .sm {
    --btn-height: var(--control-sm);
    --btn-pad: var(--space-12);
    --btn-gap: var(--space-6);
    --btn-type: var(--type-sm);
  }

  .md {
    --btn-height: var(--control-md);
    --btn-pad: var(--space-16);
    --btn-gap: var(--space-8);
    --btn-type: var(--type-md);
  }

  .lg {
    --btn-height: var(--control-lg);
    --btn-pad: var(--space-20);
    --btn-gap: var(--space-8);
    --btn-type: var(--type-md);
  }

  .icon-only {
    width: var(--btn-height);
    padding: 0;
  }

  .wide {
    width: 100%;
  }
</style>
