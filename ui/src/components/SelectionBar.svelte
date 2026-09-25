<!--
  The slim bar that replaces the list header's second row while two or more jobs are
  selected: "3 ausgewählt", the place's actions as icon buttons (the same icons, names and
  order as on a row and in the reader), and "Auswahl aufheben" at the end. Esc clears the
  selection too (outside fields), like in a mail app. It fades in (100 ms) when the second
  job joins the selection and is gone at once when the selection ends.
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface SelectionAction {
    icon: IconName;
    label: string;
    disabled?: boolean;
    disabledReason?: string | null;
    testid?: string;
    onclick: () => void;
  }
</script>

<script lang="ts">
  import { t } from '$lib/i18n/t';
  import { escape } from '$lib/input/input';
  import { fade } from '$lib/motion/transitions';
  import Button from './Button.svelte';

  interface Props {
    /** How many jobs are selected. */
    count: number;
    actions: readonly SelectionAction[];
    onclear: () => void;
    testid?: string | null;
  }

  let { count, actions, onclear, testid = null }: Props = $props();
</script>

<div
  class="bar"
  role="toolbar"
  aria-label={t.selection.count(count)}
  data-testid={testid ?? undefined}
  use:escape={onclear}
  in:fade
>
  <span class="count" data-testid="selection-count">{t.selection.count(count)}</span>
  <span class="actions">
    {#each actions as action (action.label)}
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon={action.icon}
        label={action.label}
        disabled={action.disabled ?? false}
        disabledReason={action.disabledReason ?? null}
        testid={action.testid ?? null}
        onclick={action.onclick}
      />
    {/each}
  </span>
  <span class="clear">
    <Button
      variant="ghost"
      size="sm"
      icon="x"
      label={t.selection.clear}
      testid="selection-clear"
      onclick={onclear}
    />
  </span>
</div>

<style>
  /* As wide as the row it takes, so "Auswahl aufheben" ends on its edge. */
  .bar {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
    height: var(--control-sm);
  }

  .count {
    flex: none;
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
    font-variant-numeric: var(--numeric);
  }

  .actions {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .clear {
    display: inline-flex;
    flex: none;
    margin-right: calc(-1 * var(--space-12));
  }
</style>
