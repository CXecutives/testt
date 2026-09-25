<!--
  A small fixed choice as a row of toggle buttons (the design system's pressed secondary
  button, as the filter chips): one (`multiple` off, pressing the chosen one clears it) or
  several. For the language level, the remote share and the availability of the profile: no
  dropdowns. The buttons are as tall as the fields beside them, in the small type of chips
  and segments (size field). An option may explain
  itself in a tooltip (what a language level means).
  One choice is a radiogroup like the segments: one Tab stop (the chosen option, else the
  first) and the arrows, Home and End choose (input.ts); Space on the chosen one clears it.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import { tooltip } from '$lib/actions/tooltip';

  interface Props {
    options: readonly { id: string; label: string; hint?: string | null }[];
    selected: readonly string[];
    label: string;
    multiple?: boolean;
    testid?: string | null;
    onchange: (selected: string[]) => void;
  }

  let { options, selected, label, multiple = false, testid = null, onchange }: Props = $props();

  /** One Tab stop for one choice: the chosen option, else the first. */
  const stop = $derived(
    options.find((option) => selected.includes(option.id))?.id ?? options[0]?.id ?? null,
  );

  function toggle(id: string): void {
    const on = selected.includes(id);
    if (multiple) {
      onchange(on ? selected.filter((s) => s !== id) : [...selected, id]);
    } else {
      onchange(on ? [] : [id]);
    }
  }
</script>

<div
  class="choices"
  role={multiple ? 'group' : 'radiogroup'}
  aria-label={label}
  data-testid={testid ?? undefined}
>
  {#each options as option (option.id)}
    <span
      class="choice"
      use:tooltip={option.hint && option.hint !== option.label ? option.hint : null}
    >
      <Button
        variant="secondary"
        size="field"
        label={option.label}
        pressed={multiple ? selected.includes(option.id) : null}
        radio={multiple
          ? null
          : { checked: selected.includes(option.id), stop: option.id === stop }}
        onclick={() => toggle(option.id)}
      />
    </span>
  {/each}
</div>

<style>
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    min-width: 0;
  }

  .choice {
    display: inline-flex;
  }
</style>
