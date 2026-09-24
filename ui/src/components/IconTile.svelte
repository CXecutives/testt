<!-- A soft tinted tile holding an icon or a portal monogram (in / fd / fm). Data sources
     (portals, the profile file) take the navy tone. -->
<script lang="ts" module>
  import type { Portal } from '$lib/ipc/types';

  export type TileTone = 'coral' | 'navy' | 'success' | 'warning' | 'danger' | 'neutral';
  export type TileSize = 'sm' | 'md' | 'lg';
  export const TILE_TONES: readonly TileTone[] = [
    'coral',
    'navy',
    'success',
    'warning',
    'danger',
    'neutral',
  ];

  /** Two-letter marks of the portals (brand-neutral, no logos). */
  export const PORTAL_MONOGRAM: Record<Portal, string> = {
    linkedin: 'in',
    freelance: 'fd',
    freelancermap: 'fm',
  };
</script>

<script lang="ts">
  import Icon, { type IconName, type IconSize } from './Icon.svelte';

  interface Props {
    tone?: TileTone;
    size?: TileSize;
    icon?: IconName | null;
    monogram?: string | null;
  }

  let { tone = 'neutral', size = 'md', icon = null, monogram = null }: Props = $props();

  const ICON_SIZE: Record<TileSize, IconSize> = { sm: 'sm', md: 'md', lg: 'lg' };
</script>

<span class="tile {tone} {size}" aria-hidden="true">
  {#if icon}
    <Icon name={icon} size={ICON_SIZE[size]} />
  {:else if monogram}
    <span class="monogram">{monogram}</span>
  {/if}
</span>

<style>
  .tile {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--tile-size);
    height: var(--tile-size);
    border-radius: var(--tile-radius);
    background-color: var(--tile-bg);
    color: var(--tile-fg);
  }

  .monogram {
    font: var(--tile-type);
    font-weight: var(--weight-medium);
    letter-spacing: var(--tracking-tight);
  }

  .coral {
    --tile-bg: var(--accent-soft);
    --tile-fg: var(--accent-text);
  }

  .navy {
    --tile-bg: var(--active-surface);
    --tile-fg: var(--active-text);
  }

  .success {
    --tile-bg: var(--success-soft);
    --tile-fg: var(--success-strong);
  }

  .warning {
    --tile-bg: var(--warning-soft);
    --tile-fg: var(--warning-strong);
  }

  .danger {
    --tile-bg: var(--danger-soft);
    --tile-fg: var(--danger-strong);
  }

  /* The track tone reads on white cards and on the cream page alike. */
  .neutral {
    --tile-bg: var(--surface-track);
    --tile-fg: var(--text-muted);
  }

  .sm {
    --tile-size: var(--tile-sm);
    --tile-radius: var(--radius-sm);
    --tile-type: var(--type-xs);
  }

  .md {
    --tile-size: var(--tile-md);
    --tile-radius: var(--radius-md);
    --tile-type: var(--type-sm);
  }

  .lg {
    --tile-size: var(--tile-lg);
    --tile-radius: var(--radius-lg);
    --tile-type: var(--type-lg);
  }
</style>
