<!--
  One job in the list, mail-style with fixed gutters: the unread dot (6 px, coral) centred
  in the pane padding on the axis of the ring (so a title never moves when the job is
  read), the ring, then three lines that use the full width: the title on up to two lines
  with the relative date at the end of its first line, company and place, and one line
  with the ad's key facts ("ab sofort · 6 Monate · 60 % remote · 1.100 €/Tag"; the best
  met requirement when the ad states none) and a badge right after it only when something
  deviates. Facts are whole: one that does not fit drops out, none is ever cut in the
  middle of its value. Without a usable profile the ring stays, empty (a dash), and the row
  has no third line unless a badge needs one.
  Like Mail and Gmail, the row's tools sit over the date: on hover (or when a tool has the
  keyboard focus) the date fades out and archive (or bring back) and the star fade in
  (100 ms); the title line keeps their room free. A pinned job shows a small star just
  left of the date. The tools are siblings of the row button, so they never select the row;
  the row keeps its hover while the pointer is on them. An excluded row is muted as a
  whole, its dot and tools too. When a job is read while its row is on screen the dot
  shrinks away; an excluded row has no dot (no count includes it). A date older than ten
  days sits on a quiet tint. A score from a teaser is a provisional ring. A cut-off title
  shows in full in a tooltip. Layout stays inside the row (containment); like the row, its
  hover waits while the list scrolls (`:root:not([data-scrolling])`).
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';
  /** How a click on a row selects: alone, toggled into a selection, or as a range. */
  export interface SelectHow {
    toggle: boolean;
    range: boolean;
  }

  /** A tool of the row (the job's actions where it is: archive, delete, restore ...). */
  export interface RowTool {
    id: string;
    icon: IconName;
    label: string;
    onclick: () => void;
  }
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { displayTitle, formatRelative } from '$lib/i18n/format';
  import { factWords, rowReason } from '$lib/i18n/texts';
  import type { JobView } from '$lib/ipc/types';
  import { dotOut } from '$lib/motion/transitions';
  import { keyConventions } from '$lib/platform';
  import Badge, { type BadgeTone } from './Badge.svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import ListRow from './ListRow.svelte';
  import ReasonItem from './ReasonItem.svelte';
  import ScoreRing, { ringState } from './ScoreRing.svelte';

  interface Props {
    job: JobView;
    selected?: boolean;
    /** Scoring is still running for this job. */
    pending?: boolean;
    /** A usable profile is there (without one the ring is an empty placeholder: no match). */
    ring?: boolean;
    /** Fixed "now" for relative dates (gallery and tests). */
    now?: Date;
    /** A click on the row; `how` says whether it toggles the job in a selection
     *  (Ctrl on Windows, Cmd on macOS) or selects the range up to it (Shift). */
    onselect?: ((job: JobView, how: SelectHow) => void) | null;
    /** Pin or unpin from the row; without it a pinned job only shows the star. */
    onpin?: ((job: JobView) => void) | null;
    /** Archive (or bring back an archived job) from the row. */
    onarchive?: ((job: JobView) => void) | null;
    /** The job's actions where it is, in their one order, before the star (in place of
     *  `onarchive`). */
    tools?: readonly RowTool[];
    /** The date is older than ten days (null: decide from the date and `now`). */
    aged?: boolean | null;
    /** The row's test id (another list of the same jobs needs its own). */
    testid?: string | null;
  }

  let {
    job,
    selected = false,
    pending = false,
    ring = true,
    now,
    onselect = null,
    onpin = null,
    onarchive = null,
    tools = [],
    aged = null,
    testid = null,
  }: Props = $props();

  /** A date this old is marked (days). */
  const AGED_DAYS = 10;
  const DAY_MS = 86_400_000;

  /** How a click selects, by the modifiers of the OS (like a mail app). */
  function how(event: MouseEvent): SelectHow {
    return { toggle: event[keyConventions().command], range: event.shiftKey };
  }

  const excluded = $derived(job.match?.status === 'excluded');
  const when = $derived(job.mailDate ?? job.firstSeenAt);
  const old = $derived(
    aged ?? (now ?? new Date()).getTime() - new Date(when).getTime() > AGED_DAYS * DAY_MS,
  );
  const rowId = $derived(testid ?? `job-row-${job.key.portal}-${job.key.id}`);
  /** How many tools the row has on hover (their room stays free on the title line). */
  const toolCount = $derived(tools.length + (onpin ? 1 : 0) + (onarchive ? 1 : 0));
  const reason = $derived(ring ? rowReason(job) : null);
  const facts = $derived(ring ? factWords(job.match?.facts) : []);
  const heading = $derived(job.title ? displayTitle(job.title) : t.job.untitled);

  /** At most one badge, and only when something is not as usual. */
  const deviation = $derived.by(
    (): { label: string; tone: BadgeTone; hint: string | null } | null => {
      // Excluded rows speak through the ring, the grey and the divider.
      if (excluded) return null;
      const detail = job.detail.kind;
      // While a run brings the details, "Details folgen" is no deviation.
      if (detail === 'pending' && pending) return null;
      if (detail !== 'ok') {
        const tone: BadgeTone = detail === 'teaser' || detail === 'pending' ? 'neutral' : 'warning';
        return { label: t.job.detail[detail], tone, hint: t.job.detailHint[detail] };
      }
      if (job.match?.status === 'unscorable') {
        return { label: t.score.unscorable, tone: 'neutral', hint: null };
      }
      return null;
    },
  );
</script>

{#snippet ringCell()}
  <ScoreRing
    ring={ring ? ringState(job.match, pending, job.detail.kind) : { status: 'off' }}
    size="sm"
  />
{/snippet}

<div class="job" class:tooled={toolCount > 0} class:muted={excluded}>
  <ListRow
    leading={ringCell}
    {selected}
    muted={excluded}
    onclick={onselect ? (event) => onselect?.(job, how(event)) : null}
    testid={rowId}
  >
    <span class="head">
      <span class="title" class:unread={job.unread} use:tooltip={{ text: heading, truncated: true }}
        >{heading}</span
      >
      <span
        class="end"
        class:one={toolCount === 1}
        class:two={toolCount === 2}
        class:three={toolCount >= 3}
      >
        {#if job.pinned}<span class="mark" role="img" aria-label={t.job.pinned}
            ><Icon name="star" size="sm" filled /></span
          >{/if}
        <span class="date" class:old>{formatRelative(when, now, true)}</span>
      </span>
    </span>
    <span class="meta">
      {#if job.company}<span class="text company">{job.company}</span>{/if}
      {#if job.location}<span class="text place">{job.location}</span>{/if}
    </span>
    {#if ring || facts.length > 0 || reason || deviation}<span class="foot">
        {#if facts.length > 0}
          <span class="facts" data-testid="row-facts"
            >{#each facts as fact, index (index)}<span class="fact">{fact}</span>{/each}</span
          >
        {:else if reason}
          <span class="reason"><ReasonItem kind={reason.kind} label={reason.text} compact /></span>
        {/if}
        {#if deviation}<Badge
            label={deviation.label}
            tone={deviation.tone}
            hint={deviation.hint}
          />{/if}
      </span>{/if}
  </ListRow>
  {#if job.unread && !excluded}<span class="dot" role="img" aria-label={t.job.unread} out:dotOut
    ></span>{/if}
  {#if toolCount > 0}
    <span class="tools">
      {#each tools as tool (tool.id)}
        <span class="tool">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon={tool.icon}
            label={tool.label}
            testid="{tool.id}-{job.key.portal}-{job.key.id}"
            onclick={tool.onclick}
          />
        </span>
      {/each}
      {#if onarchive}
        <span class="tool">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon={job.place === 'archive' ? 'archive-restore' : 'archive'}
            label={job.place === 'archive' ? t.reader.restore : t.reader.archive}
            testid="archive-{job.key.portal}-{job.key.id}"
            onclick={() => onarchive?.(job)}
          />
        </span>
      {/if}
      {#if onpin}
        <span class="tool">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="star"
            label={job.pinned ? t.reader.unpin : t.reader.pin}
            pressed={job.pinned}
            testid="pin-{job.key.portal}-{job.key.id}"
            onclick={() => onpin?.(job)}
          />
        </span>
      {/if}
    </span>
  {/if}
</div>

<style>
  .job {
    position: relative;
    /* Layout containment only: paint containment gave every row a clip of its own, and the
       compositor's work each frame grows with such nodes (a long list, long frames). */
    contain: layout;
  }

  /* The row keeps its hover while the pointer is on its star (a sibling of the row). */
  :global(:where(:root:not([data-scrolling]))) .job:hover :global(.row:not(.selected, :active)) {
    background-color: var(--surface-hover);
  }

  :global(:where(:root:not([data-scrolling]))) .job:hover :global(.row.selected) {
    background-color: var(--surface-selected-hover);
  }

  /* The unread dot: centred in the pane padding, on the axis of the ring. */
  .dot {
    position: absolute;
    top: calc(var(--space-12) + (var(--ring-sm) - var(--dot-unread)) / 2);
    left: calc((var(--pane-padding) - var(--dot-unread)) / 2);
    width: var(--dot-unread);
    height: var(--dot-unread);
    border-radius: var(--radius-full);
    background-color: var(--unread);
    pointer-events: none;
  }

  /* The title line: the title, and at the end of its first line the date (with a pinned
     star before it), in a room as wide as the tools that replace it on hover. */
  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
    min-width: 0;
  }

  /* A long title takes a second line (the row grows by one line); past that it ends in an
     ellipsis and shows in full in a tooltip. */
  .title {
    display: -webkit-box;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font: var(--type-title);
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .title.unread {
    font-weight: var(--weight-semibold);
  }

  .meta {
    display: flex;
    align-items: baseline;
    min-width: 0;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  /* Parts joined by a middle dot. */
  .meta > * + *::before {
    padding: 0 var(--space-6);
    color: var(--text-subtle);
    content: '·';
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The place is short and says more than the end of a long company name: the company
     gives way first. */
  .company {
    flex: 0 1 auto;
    min-width: 0;
  }

  .place {
    flex: none;
    max-width: 40%;
  }

  .end {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-4);
    height: var(--leading-title);
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  .end.one {
    min-width: var(--control-sm);
  }

  .end.two {
    min-width: calc(2 * var(--control-sm) + var(--space-2));
  }

  .end.three {
    min-width: calc(3 * var(--control-sm) + 2 * var(--space-2));
  }

  /* A pinned job: a small star just left of the date. */
  .mark {
    display: inline-flex;
    color: var(--pressed);
  }

  /* The relative date at the end of the title line; it steps up from subtle to muted on
     hover. */
  .date {
    color: var(--text-subtle);
    font: var(--type-xs);
    line-height: var(--leading-title);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
    transition: color var(--dur-base) var(--ease-standard);
  }

  :global(:where(:root:not([data-scrolling]))) .job:hover .date {
    color: var(--text-muted);
    transition-duration: var(--dur-hover);
  }

  /* One line of 20 px for every row: the reason, the badge right after it. */
  .foot {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
    height: var(--leading-title);
  }

  /* The ad's key facts, joined by middle dots, in the order of their weight (start,
     months, remote, rate). Only whole facts: one that does not fit wraps onto a second line
     that is never shown, so no value is cut ("1.100 €/Tag", never "1..."). One fact wider
     than the whole line ends in an ellipsis. */
  .facts {
    display: flex;
    flex: 0 1 auto;
    flex-wrap: wrap;
    align-content: flex-start;
    min-width: 0;
    height: var(--leading-sm);
    overflow: hidden;
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .fact {
    flex: none;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fact + .fact::before {
    padding: 0 var(--space-6);
    color: var(--text-subtle);
    content: '·';
  }

  .reason {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
  }

  /* An old date sits on a quiet tint (older than ten days). */
  .date.old {
    padding: 0 var(--space-6);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text-muted);
  }

  /* The tools over the date, centred on the title line: they fade in on hover (100 ms)
     while the date fades out. */
  .tools {
    position: absolute;
    top: calc(var(--space-12) + (var(--leading-title) - var(--control-sm)) / 2);
    right: var(--pane-padding);
    display: flex;
    gap: var(--space-2);
  }

  .tool {
    display: inline-flex;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  /* On the washed row a tool's own hover is one step deeper. */
  .tool :global(.btn.ghost) {
    --btn-bg-hover: var(--surface-press);
  }

  :global(:where(:root:not([data-scrolling]))) .job:hover .tool,
  .tool:has(:global(:focus-visible)) {
    opacity: 1;
  }

  :global(:where(:root:not([data-scrolling]))) .muted:hover .tool,
  .muted .tool:has(:global(:focus-visible)) {
    opacity: var(--opacity-muted);
  }

  :global(:where(:root:not([data-scrolling]))) .tooled:hover .end,
  .tooled:has(.tool :global(:focus-visible)) .end {
    opacity: 0;
    transition-duration: var(--dur-fast);
  }
</style>
