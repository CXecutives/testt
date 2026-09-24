<!--
  An empty screen or section never looks dead: a centred column with the app mark (or a
  meaningful icon), an optional short heading, one sentence and one clear action
  (plus at most one secondary).
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface EmptyAction {
    label: string;
    icon?: IconName;
    onclick: () => void;
  }
</script>

<script lang="ts">
  import BrandMark from './BrandMark.svelte';
  import Button from './Button.svelte';
  import IconTile, { type TileTone } from './IconTile.svelte';

  interface Props {
    /** A meaningful icon; without one the app mark is shown. */
    icon?: IconName | null;
    tone?: TileTone;
    heading?: string | null;
    text: string;
    /** The one primary action of the state. */
    action?: EmptyAction | null;
    secondary?: EmptyAction | null;
    testid?: string | null;
  }

  let {
    icon = null,
    tone = 'coral',
    heading = null,
    text,
    action = null,
    secondary = null,
    testid = null,
  }: Props = $props();
</script>

<div class="empty" data-testid={testid ?? undefined}>
  {#if icon}
    <IconTile {icon} {tone} size="lg" />
  {:else}
    <BrandMark size="lg" />
  {/if}
  <div class="copy">
    {#if heading}<h2 class="heading">{heading}</h2>{/if}
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
