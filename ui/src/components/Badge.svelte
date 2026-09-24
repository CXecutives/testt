<!-- A short status word in a pill (12/600). -->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export type BadgeTone = 'neutral' | 'coral' | 'success' | 'warning' | 'danger' | 'info';
  export const BADGE_TONES: readonly BadgeTone[] = [
    'neutral',
    'coral',
    'success',
    'warning',
    'danger',
    'info',
  ];
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import Icon from './Icon.svelte';

  interface Props {
    label: string;
    tone?: BadgeTone;
    icon?: IconName | null;
    hint?: string | null;
  }

  let { label, tone = 'neutral', icon = null, hint = null }: Props = $props();
</script>

<span class="badge {tone}" use:tooltip={hint}>
  {#if icon}<Icon name={icon} size="sm" />{/if}
  <span class="label">{label}</span>
</span>

<style>
  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-4);
    height: var(--badge-height);
    padding: 0 var(--space-8);
    border-radius: var(--radius-full);
    background-color: var(--badge-bg);
    color: var(--badge-fg);
    font: var(--type-xs);
    font-weight: var(--weight-semibold);
    white-space: nowrap;
  }

  .neutral {
    --badge-bg: var(--surface-muted);
    --badge-fg: var(--text-muted);
  }

  .coral {
    --badge-bg: var(--accent-soft);
    --badge-fg: var(--accent-text);
  }

  .success {
    --badge-bg: var(--success-soft);
    --badge-fg: var(--success-strong);
  }

  .warning {
    --badge-bg: var(--warning-soft);
    --badge-fg: var(--warning-strong);
  }

  .danger {
    --badge-bg: var(--danger-soft);
    --badge-fg: var(--danger-strong);
  }

  .info {
    --badge-bg: var(--info-soft);
    --badge-fg: var(--info-strong);
  }
</style>
