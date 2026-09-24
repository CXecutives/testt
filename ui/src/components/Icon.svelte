<!--
  The only importer of @lucide/svelte. A closed set of names, one import per icon (keeps the
  bundle small), size from the tokens, colour inherited from the text.
-->
<script lang="ts" module>
  import Archive from '@lucide/svelte/icons/archive';
  import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUpDown from '@lucide/svelte/icons/arrow-up-down';
  import Ban from '@lucide/svelte/icons/ban';
  import Briefcase from '@lucide/svelte/icons/briefcase';
  import Building2 from '@lucide/svelte/icons/building-2';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import ClipboardPaste from '@lucide/svelte/icons/clipboard-paste';
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import Circle from '@lucide/svelte/icons/circle';
  import CircleDashed from '@lucide/svelte/icons/circle-dashed';
  import CircleDot from '@lucide/svelte/icons/circle-dot';
  import CircleQuestionMark from '@lucide/svelte/icons/circle-question-mark';
  import CircleStop from '@lucide/svelte/icons/circle-stop';
  import Clock from '@lucide/svelte/icons/clock';
  import Contrast from '@lucide/svelte/icons/contrast';
  import Copy from '@lucide/svelte/icons/copy';
  import Download from '@lucide/svelte/icons/download';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import FileText from '@lucide/svelte/icons/file-text';
  import FileUp from '@lucide/svelte/icons/file-up';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Inbox from '@lucide/svelte/icons/inbox';
  import Info from '@lucide/svelte/icons/info';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import LogIn from '@lucide/svelte/icons/log-in';
  import LogOut from '@lucide/svelte/icons/log-out';
  import Mail from '@lucide/svelte/icons/mail';
  import MapPin from '@lucide/svelte/icons/map-pin';
  import Minus from '@lucide/svelte/icons/minus';
  import PauseCircle from '@lucide/svelte/icons/pause-circle';
  import Plus from '@lucide/svelte/icons/plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Search from '@lucide/svelte/icons/search';
  import Shield from '@lucide/svelte/icons/shield';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import Star from '@lucide/svelte/icons/star';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import UserRound from '@lucide/svelte/icons/user-round';
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
    'clipboard-paste': ClipboardPaste,
    'trash-2': Trash2,
    'rotate-ccw': RotateCcw,
    'log-in': LogIn,
    'log-out': LogOut,
    'key-round': KeyRound,
    'chevron-down': ChevronDown,
    'chevron-left': ChevronLeft,
    'chevron-up': ChevronUp,
    'external-link': ExternalLink,
    mail: Mail,
    'file-text': FileText,
    'map-pin': MapPin,
    'building-2': Building2,
    clock: Clock,
    check: Check,
    'circle-dashed': CircleDashed,
    'triangle-alert': TriangleAlert,
    ban: Ban,
    info: Info,
    'pause-circle': PauseCircle,
    plus: Plus,
    'wifi-off': WifiOff,
    minus: Minus,
    // The sort toggle of the list, the cancel of a run, the steps of a run.
    'arrow-up-down': ArrowUpDown,
    'circle-stop': CircleStop,
    'loader-circle': LoaderCircle,
    circle: Circle,
    star: Star,
    shield: Shield,
    // Added for the password field (show / hide).
    eye: Eye,
    'eye-off': EyeOff,
    // Empty states (the user ruled out "sparkles" as an AI cliché).
    inbox: Inbox,
    // Sidebar navigation and toasts.
    briefcase: Briefcase,
    'user-round': UserRound,
    'sliders-horizontal': SlidersHorizontal,
    'circle-check': CircleCheck,
    // Criteria chips: a point the ad leaves open ("zu prüfen").
    'circle-help': CircleQuestionMark,
    // A requirement met only in part: half a circle, next to the full check and the empty ring.
    'circle-half': Contrast,
    // The current step of a run (the header spinner is the one moving indicator).
    'circle-dot': CircleDot,
    // A reason that jumps to its passage in the ad (shown on hover).
    'arrow-down': ArrowDown,
    // Archive a job from its row, and bring an archived one back.
    archive: Archive,
    'archive-restore': ArchiveRestore,
  } satisfies Record<string, Component<Record<string, unknown>>>;

  export type IconName = keyof typeof ICONS;
  export type IconSize = 'xs' | 'sm' | 'md' | 'lg';
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

  .xs {
    --icon-size: var(--icon-xs);
    --icon-stroke: var(--icon-stroke-xs);
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
