<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    heading: string;
    id: string;
    children: Snippet;
  }

  let { heading, id, children }: Props = $props();
</script>

<section class="section" data-testid="gallery-{id}">
  <h2 class="heading">{heading}</h2>
  <div class="body">{@render children()}</div>
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    padding: var(--space-24);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
    box-shadow: var(--sh-sm);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-xl);
    letter-spacing: var(--tracking-tight);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
  }
</style>
