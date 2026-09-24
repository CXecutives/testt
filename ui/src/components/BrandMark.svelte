<!-- The app's own icon (coral plate, white folder, check mark): xs 16 in the Windows title
     bar (like the small icon of a native window), larger on the first run and empty states. -->
<script lang="ts">
  import icon from '../assets/app-icon.svg';

  interface Props {
    size?: 'xs' | 'sm' | 'md' | 'lg';
    /** Accessible name; omit when the app name stands next to the mark. */
    label?: string | null;
  }

  let { size = 'md', label = null }: Props = $props();
</script>

<img
  class="mark {size}"
  src={icon}
  alt={label ?? ''}
  aria-hidden={label ? undefined : 'true'}
  draggable="false"
/>

<style>
  .mark {
    flex: none;
    width: var(--mark-size);
    height: var(--mark-size);
    pointer-events: none;
  }

  .xs {
    --mark-size: var(--brand-mark-xs);
  }

  .sm {
    --mark-size: var(--brand-mark-sm);
  }

  .md {
    --mark-size: var(--brand-mark);
  }

  .lg {
    --mark-size: var(--brand-mark-lg);
  }
</style>
