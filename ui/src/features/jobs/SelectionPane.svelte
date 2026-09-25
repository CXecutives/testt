<!--
  The reader while two or more jobs are chosen (like Mail and Outlook): no single job, only
  how many are chosen and one quiet line on how to choose. What can be done with them is said
  once, in the list header's selection bar.
-->
<script lang="ts">
  import { t } from '$lib/i18n/t';
  import { commandKey } from '$lib/platform';
  import { bulk } from './bulk.svelte';
</script>

<section
  class="pane"
  data-testid="selection-pane"
  aria-label={t.selection.chosen(bulk.chosen.length)}
>
  <h2 class="heading">{t.selection.chosen(bulk.chosen.length)}</h2>
  <p class="hint">{t.selection.hint(t.selection.commandKey[commandKey()])}</p>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
