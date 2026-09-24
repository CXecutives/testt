<!--
  A small fixed choice as a row of toggle buttons (the design system's pressed secondary
  button, as the filter chips): one (`multiple` off, pressing the chosen one clears it) or
  several. For the language level and the countries of the profile: no dropdowns.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';

  interface Props {
    options: readonly { id: string; label: string }[];
    selected: readonly string[];
    label: string;
    multiple?: boolean;
    testid?: string | null;
    onchange: (selected: string[]) => void;
  }

  let { options, selected, label, multiple = false, testid = null, onchange }: Props = $props();

  function toggle(id: string): void {
    const on = selected.includes(id);
    if (multiple) {
      onchange(on ? selected.filter((s) => s !== id) : [...selected, id]);
    } else {
      onchange(on ? [] : [id]);
    }
  }
</script>

<div class="choices" role="group" aria-label={label} data-testid={testid ?? undefined}>
  {#each options as option (option.id)}
    <Button
      variant="secondary"
      size="sm"
      label={option.label}
      pressed={selected.includes(option.id)}
      onclick={() => toggle(option.id)}
    />
  {/each}
</div>

<style>
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    min-width: 0;
  }
</style>
