<!--
  The reader while two or more jobs are chosen (like Mail and Outlook): no single job, only
  how many are chosen and what can be done with all of them, as labelled buttons (the same
  actions as the selection bar), "Auswahl aufheben", and one quiet line on how to choose.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import { t } from '$lib/i18n/t';
  import { commandKey } from '$lib/platform';
  import { bulk } from './bulk.svelte';
  import { selection } from './selection.svelte';
</script>

<section
  class="pane"
  data-testid="selection-pane"
  aria-label={t.selection.chosen(bulk.chosen.length)}
>
  <h2 class="heading">{t.selection.chosen(bulk.chosen.length)}</h2>
  <div class="actions">
    {#each bulk.actions as action (action.label)}
      <Button
        variant="secondary"
        icon={action.icon}
        label={action.label}
        disabled={action.disabled ?? false}
        disabledReason={action.disabledReason ?? null}
        testid="pane-{action.testid ?? action.label}"
        onclick={action.onclick}
      />
    {/each}
    <Button
      variant="ghost"
      icon="x"
      label={t.selection.clear}
      testid="pane-clear"
      onclick={() => selection.clear()}
    />
  </div>
  <p class="hint">{t.selection.hint(t.selection.commandKey[commandKey()])}</p>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
