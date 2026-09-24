<!--
  The only importer of @lucide/svelte. A closed set of names, one import per icon (keeps the
  bundle small), size from the tokens, colour inherited from the text.
-->
<script lang="ts" module>
  import Ban from '@lucide/svelte/icons/ban';
  import Building2 from '@lucide/svelte/icons/building-2';
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import CircleDashed from '@lucide/svelte/icons/circle-dashed';
  import Clock from '@lucide/svelte/icons/clock';
  import Copy from '@lucide/svelte/icons/copy';
  import Download from '@lucide/svelte/icons/download';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import FileText from '@lucide/svelte/icons/file-text';
  import FileUp from '@lucide/svelte/icons/file-up';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Info from '@lucide/svelte/icons/info';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import LogIn from '@lucide/svelte/icons/log-in';
  import LogOut from '@lucide/svelte/icons/log-out';
  import Mail from '@lucide/svelte/icons/mail';
  import MapPin from '@lucide/svelte/icons/map-pin';
  import Minus from '@lucide/svelte/icons/minus';
  import PauseCircle from '@lucide/svelte/icons/pause-circle';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Search from '@lucide/svelte/icons/search';
  import Shield from '@lucide/svelte/icons/shield';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import Square from '@lucide/svelte/icons/square';
  import Star from '@lucide/svelte/icons/star';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import WifiOff from '@lucide/svelte/icons/wifi-off';
  import X from '@lucide/svelte/icons/x';
  import type { Component } from 'svelte';

  const ICONS = {
    'refresh-cw': RefreshCw,
    x: X,
    search: Search,
    download: Download,
    'folder-open': FolderOpen,
    'file-up': FileUp,
    copy: Copy,
    'trash-2': Trash2,
    'rotate-ccw': RotateCcw,
    'log-in': LogIn,
    'log-out': LogOut,
    'key-round': KeyRound,
    'chevron-down': ChevronDown,
    'chevron-left': ChevronLeft,
    'external-link': ExternalLink,
    mail: Mail,
    'file-text': FileText,
    'map-pin': MapPin,
    'building-2': Building2,
    clock: Clock,
    sparkles: Sparkles,
    check: Check,
    'circle-dashed': CircleDashed,
    'triangle-alert': TriangleAlert,
    ban: Ban,
    info: Info,
    'pause-circle': PauseCircle,
    'wifi-off': WifiOff,
    minus: Minus,
    square: Square,
    star: Star,
    shield: Shield,
    // Added for the password field (show / hide).
    eye: Eye,
    'eye-off': EyeOff,
  } satisfies Record<string, Component<Record<string, unknown>>>;

  export type IconName = keyof typeof ICONS;
  export type IconSize = 'sm' | 'md' | 'lg';
  export const ICON_NAMES = Object.keys(ICONS) as IconName[];
</script>

<script lang="ts">
  interface Props {
    name: IconName;
    size?: IconSize;
    /** Filled glyph (the pinned star). */
    filled?: boolean;
  }

  let { name, size = 'md', filled = false }: Props = $props();

  const Glyph = $derived(ICONS[name]);
</script>

<span class="icon {size}" class:filled aria-hidden="true">
  <Glyph aria-hidden="true" />
</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-size);
    height: var(--icon-size);
    color: inherit;
  }

  .icon :global(svg) {
    width: 100%;
    height: 100%;
    stroke-width: var(--icon-stroke);
  }

  .filled :global(svg) {
    fill: currentcolor;
  }

  .sm {
    --icon-size: var(--icon-sm);
    --icon-stroke: var(--icon-stroke-sm);
  }

  .md {
    --icon-size: var(--icon-md);
    --icon-stroke: var(--icon-stroke-md);
  }

  .lg {
    --icon-size: var(--icon-lg);
    --icon-stroke: var(--icon-stroke-lg);
  }
</style>
