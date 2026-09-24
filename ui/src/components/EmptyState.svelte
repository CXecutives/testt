<!-- An empty screen or section: tile, heading, one sentence, at most two actions. -->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface EmptyAction {
    label: string;
    icon?: IconName;
    onclick: () => void;
  }
</script>

<script lang="ts">
  import Button from './Button.svelte';
  import IconTile, { type TileTone } from './IconTile.svelte';

  interface Props {
    icon: IconName;
    tone?: TileTone;
    heading: string;
    text: string;
    /** The one primary action of the state. */
    action?: EmptyAction | null;
    secondary?: EmptyAction | null;
    testid?: string | null;
  }

  let {
    icon,
    tone = 'coral',
    heading,
    text,
    action = null,
    secondary = null,
    testid = null,
  }: Props = $props();
</script>

<div class="empty" data-testid={testid ?? undefined}>
  <IconTile {icon} {tone} size="lg" />
  <div class="copy">
    <h2 class="heading">{heading}</h2>
    <p class="text">{text}</p>
  </div>
  {#if action || secondary}
    <div class="actions">
      {#if action}
        <Button
          variant="primary"
          label={action.label}
          icon={action.icon ?? null}
          onclick={action.onclick}
        />
      {/if}
      {#if secondary}
        <Button
          variant="secondary"
          label={secondary.label}
          icon={secondary.icon ?? null}
          onclick={secondary.onclick}
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-20);
    max-width: var(--form-width);
    text-align: center;
  }

  .copy {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-xl);
    letter-spacing: var(--tracking-tight);
  }

  .text {
    color: var(--text-muted);
    font: var(--type-body);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-12);
  }
</style>
