<!--
  On/off switch. The thumb pops across (ease-pop); disabled switches stay hoverable so the
  tooltip can say why (disabledReason).
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';

  interface Props {
    checked: boolean;
    label: string;
    /** Show the label next to the switch (otherwise it is only the accessible name). */
    showLabel?: boolean;
    disabled?: boolean;
    disabledReason?: string | null;
    testid?: string | null;
    onchange: (checked: boolean) => void;
  }

  let {
    checked,
    label,
    showLabel = false,
    disabled = false,
    disabledReason = null,
    testid = null,
    onchange,
  }: Props = $props();

  function toggle(): void {
    if (!disabled) onchange(!checked);
  }
</script>

<button
  type="button"
  role="switch"
  class="toggle"
  aria-checked={checked}
  aria-label={showLabel ? undefined : label}
  aria-disabled={disabled ? 'true' : undefined}
  data-testid={testid ?? undefined}
  use:tooltip={disabled ? disabledReason : null}
  onclick={toggle}
>
  <span class="track"><span class="thumb"></span></span>
  {#if showLabel}<span class="label">{label}</span>{/if}
</button>

<style>
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-12);
    color: var(--text);
    font: var(--type-md);
  }

  .track {
    position: relative;
    display: inline-block;
    flex: none;
    width: var(--toggle-width);
    height: var(--toggle-height);
    border-radius: var(--radius-full);
    background-color: var(--border-strong);
    transition: background-color var(--dur-fast) var(--ease-standard);
  }

  .thumb {
    position: absolute;
    top: calc((var(--toggle-height) - var(--toggle-thumb)) / 2);
    left: calc((var(--toggle-height) - var(--toggle-thumb)) / 2);
    width: var(--toggle-thumb);
    height: var(--toggle-thumb);
    border-radius: var(--radius-full);
    background-color: var(--surface);
    box-shadow: var(--sh-sm);
    transition: transform var(--dur-base) var(--ease-pop);
  }

  .toggle:not([aria-disabled='true']):hover .track {
    background-color: var(--border-input);
  }

  .toggle[aria-checked='true'] .track {
    background-color: var(--toggle-on);
  }

  .toggle[aria-checked='true']:not([aria-disabled='true']):hover .track {
    background-color: var(--toggle-on-hover);
  }

  .toggle[aria-checked='true'] .thumb {
    transform: translateX(var(--toggle-travel));
  }

  .toggle:not([aria-disabled='true']):active .thumb {
    transform: scale(var(--scale-press));
  }

  .toggle[aria-checked='true']:not([aria-disabled='true']):active .thumb {
    transform: translateX(var(--toggle-travel)) scale(var(--scale-press));
  }

  .toggle:focus-visible .track {
    box-shadow: var(--focus-ring);
  }

  .toggle[aria-disabled='true'] {
    cursor: default;
    opacity: var(--opacity-disabled);
  }
</style>
