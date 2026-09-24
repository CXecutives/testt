<!--
  The button of the app: primary | secondary | ghost | danger | link × sm | md | lg. Native
  in feel, rich on contact: hover-in changes colour in 80 ms and relaxes in 150 ms, the
  icon nudges toward what it does (external link up-right, download down, refresh a
  quarter turn, the star grows), a press lets the button give a little, uniformly (0.98,
  60 ms), and it settles back in 150 ms. Nothing stretches; no lift, no glow, no bounce.
  - Trailing actions inside a row are sm, action bars are md.
  - At most one primary per view (checked by core/tests/ui_contract.rs).
  - iconOnly needs its label: it becomes aria-label and tooltip.
  - Disabled buttons stay hoverable (aria-disabled) so the tooltip can say why; they do
    not react otherwise.
  - Loading keeps the width: the content fades out under the spinner.
  - A ghost toggle (the pin star) pops once when it is switched on by a click.
  - turned: the glyph stands half a turn; it turns in 180 ms.
  - link: navy text that underlines on hover (a way on, e.g. under a field).
  - inField: a button inside a text field (show password, clear search), like the native
    ones: not in the Tab order, and a click leaves the caret in the field.
  - isDefault: the default of a dialog, the one Enter presses; the dialog marks it.
  - warns: a quiet (secondary or ghost) button that removes or resets something: its text
    turns red on hover, before the dialog asks. A ghost with the trash icon always warns.
  The icon sits on its own HTML wrapper: transforms on SVG children run on the main thread.
-->
<script lang="ts" module>
  export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'link';
  export type ButtonSize = 'sm' | 'md' | 'lg';
  export const BUTTON_VARIANTS: readonly ButtonVariant[] = [
    'primary',
    'secondary',
    'ghost',
    'danger',
    'link',
  ];
  export const BUTTON_SIZES: readonly ButtonSize[] = ['sm', 'md', 'lg'];
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { fade, pulseOnce } from '$lib/motion/transitions';
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
    /** The glyph stands half a turn. */
    turned?: boolean;
    /** Opens something outside the app (a link shows the hand then). */
    external?: boolean;
    /** Fill the width of the container. */
    wide?: boolean;
    /** Sits inside a text field: skipped by Tab, a click keeps the focus in the field. */
    inField?: boolean;
    /** A glyph after the label (the chevron of a menu button). */
    trailing?: IconName | null;
    /** It opens a menu (announced as such). */
    menu?: boolean;
    /** The default of a dialog (Enter presses it); Dialog marks it with the focus ring. */
    isDefault?: boolean;
    /** Removes or resets something: red text on hover (secondary and ghost). */
    warns?: boolean;
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
    turned = false,
    external = false,
    wide = false,
    inField = false,
    trailing = null,
    menu = false,
    isDefault = false,
    warns = false,
    testid = null,
    onclick,
  }: Props = $props();

  const ICON_SIZE: Record<ButtonSize, IconSize> = { sm: 'sm', md: 'sm', lg: 'md' };
  const ICON_ONLY_SIZE: Record<ButtonSize, IconSize> = { sm: 'sm', md: 'md', lg: 'lg' };

  const inactive = $derived(disabled || loading);
  const hint = $derived(disabled && disabledReason ? disabledReason : iconOnly ? label : null);

  let glyph = $state<HTMLElement | null>(null);

  function handle(event: MouseEvent): void {
    if (inactive) {
      event.preventDefault();
      return;
    }
    // A ghost toggle that a click switches on pops once (the star when pinning).
    const switchesOn = variant === 'ghost' && pressed === false;
    onclick?.(event);
    if (switchesOn && glyph !== null) pulseOnce(glyph);
  }
</script>

<button
  {type}
  class="btn {variant} {size}"
  class:icon-only={iconOnly}
  class:wide={wide && !iconOnly}
  class:loading
  class:turned
  class:external
  class:default={isDefault}
  class:warns={warns || (variant === 'ghost' && icon === 'trash-2')}
  aria-label={iconOnly ? label : undefined}
  aria-disabled={disabled ? 'true' : undefined}
  aria-busy={loading ? 'true' : undefined}
  aria-pressed={pressed === null ? undefined : pressed}
  aria-haspopup={menu ? 'menu' : undefined}
  tabindex={inField ? -1 : undefined}
  data-keep-focus={inField ? '' : undefined}
  data-testid={testid ?? undefined}
  use:tooltip={hint}
  onclick={handle}
>
  <span class="content">
    {#if icon}
      <span class="glyph" data-icon={icon} bind:this={glyph}>
        <Icon
          name={icon}
          size={iconOnly ? ICON_ONLY_SIZE[size] : ICON_SIZE[size]}
          filled={pressed === true}
        />
      </span>
    {/if}
    {#if !iconOnly}
      <span class="label">{label}</span>
    {/if}
    {#if trailing}
      <span class="trailing" aria-hidden="true"><Icon name={trailing} size="sm" /></span>
    {/if}
  </span>
  {#if loading}
    <span class="busy" in:fade><Spinner size="sm" label={null} /></span>
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
    --btn-press: var(--scale-press);

    transition:
      background-color var(--dur-base) var(--ease-standard),
      border-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard),
      transform var(--dur-base) var(--ease-emphasized);
  }

  .content {
    display: inline-flex;
    align-items: center;
    gap: var(--btn-gap);
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  .trailing {
    display: inline-flex;
    margin-right: calc(-1 * var(--space-4));
  }

  .glyph {
    display: inline-flex;
    transition: transform var(--dur-base) var(--ease-emphasized);
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

  /* Hover-in in 80 ms; the release of a press keeps its 150 ms (the last duration). */
  .btn:not([aria-disabled='true'], .loading):hover {
    border-color: var(--btn-border-hover);
    background-color: var(--btn-bg-hover);
    color: var(--btn-fg-hover);
    transition-duration: var(--dur-hover), var(--dur-hover), var(--dur-hover), var(--dur-base);
  }

  /* Pressed: only while the left button is down (input.ts keeps the others from pressing). */
  .btn:not([aria-disabled='true'], .loading):active {
    border-color: var(--btn-border-active);
    background-color: var(--btn-bg-active);
    transform: scale(var(--btn-press));
    transition-duration: var(--dur-instant);
  }

  /* The glyph nudges toward what the button does; it holds while pressed. */
  .btn:not([aria-disabled='true'], .loading, .turned):hover .glyph {
    transition-duration: var(--dur-hover);
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='external-link'] {
    transform: translate(var(--move-xs), calc(-1 * var(--move-xs)));
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='chevron-left'] {
    transform: translateX(calc(-1 * var(--move-sm)));
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='download'] {
    transform: translateY(var(--move-xs));
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='file-up'],
  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='mail'] {
    transform: translateY(calc(-1 * var(--move-xs)));
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='refresh-cw'] {
    transform: rotate(var(--turn-nudge));
  }

  .btn:not([aria-disabled='true'], .loading, .danger):hover .glyph[data-icon='star'] {
    transform: scale(var(--scale-nudge));
  }

  /* A state, not a nudge: half a turn in 180 ms (the angle stays under reduced motion). */
  .turned .glyph {
    transform: rotate(var(--turn-half));
    transition-duration: var(--dur-slow);
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
    --btn-shadow: var(--sh-primary);
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

  /* A secondary toggle that is on (a filter chip): the navy trio of a chosen filter. */
  .secondary[aria-pressed='true'] {
    --btn-bg: var(--active-surface);
    --btn-bg-hover: var(--active-surface);
    --btn-bg-active: var(--active-surface);
    --btn-border: var(--active-edge);
    --btn-border-hover: var(--active-edge);
    --btn-border-active: var(--active-edge);
    --btn-fg: var(--active-text);
    --btn-fg-hover: var(--active-text);
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

  /* Removing or resetting something: a quiet warning on hover, before the dialog asks. */
  .ghost.warns,
  .secondary.warns {
    --btn-fg-hover: var(--danger-strong);
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
    --btn-shadow: var(--sh-primary);
  }

  /* Navy text, no box; it underlines on hover and dims while pressed (it is text). */
  .btn.link {
    --btn-bg: transparent;
    --btn-bg-hover: transparent;
    --btn-bg-active: transparent;
    --btn-border: transparent;
    --btn-border-hover: transparent;
    --btn-border-active: transparent;
    --btn-fg: var(--link);
    --btn-fg-hover: var(--link-hover);
    --btn-shadow: none;
    --btn-height: var(--control-sm);
    --btn-pad: 0;
    --btn-press: 1;

    transition:
      color var(--dur-base) var(--ease-standard),
      opacity var(--dur-base) var(--ease-standard);
  }

  .link .label {
    position: relative;
  }

  .link .label::after {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: var(--border-width);
    background-color: currentcolor;
    content: '';
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .link:not([aria-disabled='true'], .loading):hover .label::after {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  .link:not([aria-disabled='true'], .loading):active {
    opacity: var(--opacity-press);
    transition-duration: var(--dur-instant);
  }

  .link.external {
    cursor: pointer;
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
