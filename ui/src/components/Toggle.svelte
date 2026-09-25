<!--
  On/off switch, coral when on (user decision). The thumb travels in 180 ms (emphasized, no
  bounce) and the track changes colour in 100 ms; it darkens a step under the pointer and one
  more while the left button is down, off and on alike (the thumb never changes shape).
  Disabled switches stay hoverable so the tooltip can say why (disabledReason), and Tab
  passes them like native disabled controls.
  It flips at once, like a native switch: when `onchange` returns a promise (the save),
  the switch shows the new state until it settles, then `checked` again - which is the old
  state if the save failed, so the thumb slides back.
  Only the switch itself switches, like the switches of the Windows 11 and macOS settings
  (user decision): its label and the text of its row are no click target and never show a
  hover. `id` ties it to the text of its SettingRow (`for`): the row's label names it
  (`{id}-label`) and the row's hint, while it has one, describes it.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { describedBy } from '$lib/state/described';

  interface Props {
    checked: boolean;
    label: string;
    /** Show the label next to the switch (otherwise it is only the accessible name). */
    showLabel?: boolean;
    disabled?: boolean;
    disabledReason?: string | null;
    /** Ties the switch to the text of its SettingRow (`for`), which names and describes it. */
    id?: string | null;
    /** The id of a text that describes the switch outside a SettingRow (only while shown). */
    describedby?: string | null;
    testid?: string | null;
    /** Return the save's promise: the switch shows the new state until it settles. */
    onchange: (checked: boolean) => unknown;
  }

  let {
    checked,
    label,
    showLabel = false,
    disabled = false,
    disabledReason = null,
    id = null,
    describedby = null,
    testid = null,
    onchange,
  }: Props = $props();

  const described = describedBy();

  /** The state shown while a save is on its way (null: show `checked`). */
  let pending = $state<boolean | null>(null);
  const own = $props.id();
  /** The element that names the switch: the label beside it, else the text of its row. */
  const labelledby = $derived(showLabel ? `${own}-label` : id ? `${id}-label` : null);
  let attempt = 0;
  const shown = $derived(pending ?? checked);

  async function toggle(): Promise<void> {
    if (disabled) return;
    const next = !shown;
    const mine = ++attempt;
    pending = next;
    // Settled either way: the caller reports a failure, the switch shows `checked` again.
    const settle = (): void => {
      if (mine === attempt) pending = null;
    };
    await Promise.resolve(onchange(next)).then(settle, settle);
  }
</script>

{#snippet control()}
  <button
    type="button"
    role="switch"
    class="toggle"
    id={id ?? undefined}
    aria-checked={shown}
    aria-label={label}
    aria-labelledby={labelledby ?? undefined}
    aria-describedby={describedby ?? described() ?? undefined}
    aria-disabled={disabled ? 'true' : undefined}
    tabindex={disabled ? -1 : undefined}
    data-testid={testid ?? undefined}
    use:tooltip={disabled ? disabledReason : null}
    onclick={() => void toggle()}
  >
    <span class="track"><span class="thumb"></span></span>
  </button>
{/snippet}

{#if showLabel}
  <!-- The label beside the switch names it but does not switch it. -->
  <span class="with-label" class:off={disabled}>
    {@render control()}
    <span class="label" id="{own}-label">{label}</span>
  </span>
{:else}
  {@render control()}
{/if}

<style>
  .toggle {
    display: inline-flex;
    flex: none;
    align-items: center;
  }

  .with-label {
    display: inline-flex;
    align-items: center;
    gap: var(--space-12);
    color: var(--text);
    font: var(--type-md);
  }

  .with-label.off .label {
    opacity: var(--opacity-disabled);
  }

  .track {
    position: relative;
    display: inline-block;
    flex: none;
    width: var(--toggle-width);
    height: var(--toggle-height);
    border-radius: var(--radius-full);
    background-color: var(--toggle-off);
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
    box-shadow: var(--sh-thumb);
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .toggle:not([aria-disabled='true']):hover .track {
    background-color: var(--toggle-off-hover);
    transition-duration: var(--dur-hover);
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

  /* Pressed: the track darkens one more step than on hover, 60 ms, off and on alike. */
  :global(:where(:root:not([data-aux-press])))
    .toggle:not([aria-disabled='true']):active:hover
    .track {
    background-color: var(--toggle-off-press);
    transition-duration: var(--dur-instant);
  }

  :global(:where(:root:not([data-aux-press])))
    .toggle[aria-checked='true']:not([aria-disabled='true']):active:hover
    .track {
    background-color: var(--toggle-on-press);
  }

  .toggle:focus-visible .track {
    box-shadow: var(--focus-ring);
  }

  .toggle[aria-disabled='true'] {
    cursor: default;
    opacity: var(--opacity-disabled);
  }
</style>
