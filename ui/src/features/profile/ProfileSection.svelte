<!--
  One section of the profile form: the heading (with "Noch leer" when a thin profile leaves
  it empty), at most one sentence under it, the fields in a card.
-->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Card from '$components/Card.svelte';
  import { t } from '$lib/i18n/t';
  import type { Snippet } from 'svelte';

  interface Props {
    heading: string;
    hint?: string | null;
    /** Mark the section as empty (quality guidance for a thin profile). */
    empty?: boolean;
    testid?: string | null;
    children: Snippet;
  }

  let { heading, hint = null, empty = false, testid = null, children }: Props = $props();
  const id = $props.id();
</script>

<section class="section" aria-labelledby="{id}-heading" data-testid={testid ?? undefined}>
  <div class="head">
    <h2 class="heading" id="{id}-heading">{heading}</h2>
    {#if empty}<Badge label={t.profile.empty} tone="warning" />{/if}
  </div>
  {#if hint}<p class="hint">{hint}</p>{/if}
  <Card padding="md">
    <div class="fields">{@render children()}</div>
  </Card>
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .hint {
    margin-top: calc(-1 * var(--space-4));
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    container-type: inline-size;
  }
</style>
