<!--
  One job in the list, mail-style with fixed gutters: the unread dot (6 px, coral) centred
  in the pane padding on the axis of the ring (so a title never moves when the job is
  read), the ring, then the title on one line with the relative date at its end, company
  and place, and one reason line with a status badge right after it only when something
  deviates. Every row has the same height. Without a ring (no usable profile) the dot sits
  on the title axis and the row shows no reason line: the reasons belong to a match.
  The star to pin sits below the date: filled when pinned, otherwise it appears on hover (a
  sibling of the row button, so it never selects the row; the row keeps its hover while
  the pointer is on the star). An excluded row is muted as a whole, its dot and star too.
  When a job is read while its row is on screen the dot shrinks away; an excluded row has
  no dot (no count includes it). Under the date, on hover: archive (or bring back) and the
  star (a pinned star always shows). A quiet badge says where the user's application
  stands (Beworben, Im Gespräch, Zusage, Absage); pinned needs none (the star). A date older
  than ten days sits on a quiet tint. A score from a teaser is a provisional ring. A cut-off
  title shows in full in a tooltip. Hover and paint stay inside the row (containment).
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import { displayTitle, formatRelative } from '$lib/i18n/format';
  import { rowReason } from '$lib/i18n/texts';
  import type { AppStatus, JobView } from '$lib/ipc/types';
  import { dotOut } from '$lib/motion/transitions';
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
    /** Show the ring (off without a profile: there is no match to show). */
    ring?: boolean;
    /** Fixed "now" for relative dates (gallery and tests). */
    now?: Date;
    onselect?: ((job: JobView) => void) | null;
    /** Pin or unpin from the row; without it a pinned job only shows the star. */
    onpin?: ((job: JobView) => void) | null;
    /** Archive (or bring back an archived job) from the row. */
    onarchive?: ((job: JobView) => void) | null;
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
    aged = null,
    testid = null,
  }: Props = $props();

  /** A date this old is marked (days). */
  const AGED_DAYS = 10;
  const DAY_MS = 86_400_000;
  const APP_TONE: Record<AppStatus, BadgeTone> = {
    applied: 'neutral',
    interview: 'neutral',
    offer: 'success',
    rejected: 'neutral',
  };

  const excluded = $derived(job.match?.status === 'excluded');
  const when = $derived(job.mailDate ?? job.firstSeenAt);
  const old = $derived(
    aged ?? (now ?? new Date()).getTime() - new Date(when).getTime() > AGED_DAYS * DAY_MS,
  );
  const rowId = $derived(testid ?? `job-row-${job.key.portal}-${job.key.id}`);
  /** Where the user's application stands (pinned needs no badge: the star says it). */
  const status = $derived(
    job.appStatus
      ? { label: de.reader.appStatus[job.appStatus], tone: APP_TONE[job.appStatus] }
      : null,
  );
  const reason = $derived(ring ? rowReason(job) : null);
  const heading = $derived(job.title ? displayTitle(job.title) : de.job.untitled);

  /** At most one badge, and only when something is not as usual. */
  const deviation = $derived.by((): { label: string; tone: BadgeTone } | null => {
    // Excluded rows speak through the ring, the grey and the divider.
    if (excluded) return null;
    const detail = job.detail.kind;
    if (detail !== 'ok') {
      const tone: BadgeTone = detail === 'teaser' || detail === 'pending' ? 'neutral' : 'warning';
      return { label: de.job.detail[detail], tone };
    }
    if (job.match?.status === 'unscorable') return { label: de.score.unscorable, tone: 'neutral' };
    return null;
  });
</script>

{#snippet ringCell()}
  <ScoreRing ring={ringState(job.match, pending, job.detail.kind)} size="sm" />
{/snippet}

<!-- Without a ring an empty leading slot keeps the gap between the dot and the title. -->
{#snippet gutter()}
  <span class="gutter"></span>
{/snippet}

{#snippet endCell()}
  <span class="date" class:old>{formatRelative(when, now, true)}</span>
  {#if onpin || onarchive}
    <span class="tool-slot" class:two={onpin && onarchive} aria-hidden="true"></span>
  {:else if job.pinned}
    <span class="star" role="img" aria-label={de.job.pinned}
      ><Icon name="star" size="sm" filled /></span
    >
  {/if}
{/snippet}

<div class="job" class:pinned={job.pinned} class:muted={excluded} class:ringless={!ring}>
  <ListRow
    leading={ring ? ringCell : gutter}
    trailing={endCell}
    {selected}
    muted={excluded}
    onclick={onselect ? () => onselect?.(job) : null}
    testid={rowId}
  >
    <span class="title" class:unread={job.unread} use:tooltip={{ text: heading, truncated: true }}
      >{heading}</span
    >
    <span class="meta">
      {#if job.company}<span class="text company">{job.company}</span>{/if}
      {#if job.location}<span class="text place">{job.location}</span>{/if}
    </span>
    <span class="foot">
      {#if reason}
        <span class="reason"><ReasonItem kind={reason.kind} label={reason.text} compact /></span>
      {/if}
      {#if status}<Badge label={status.label} tone={status.tone} />{/if}
      {#if deviation}<Badge label={deviation.label} tone={deviation.tone} />{/if}
    </span>
  </ListRow>
  {#if job.unread && !excluded}<span class="dot" role="img" aria-label={de.job.unread} out:dotOut
    ></span>{/if}
  {#if onpin || onarchive}
    <span class="tools">
      {#if onarchive}
        <span class="tool">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon={job.hidden ? 'archive-restore' : 'archive'}
            label={job.hidden ? de.reader.unhide : de.reader.hide}
            testid="archive-{job.key.portal}-{job.key.id}"
            onclick={() => onarchive?.(job)}
          />
        </span>
      {/if}
      {#if onpin}
        <span class="tool pin">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            icon="star"
            label={de.reader.pin}
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
    contain: layout paint;
  }

  /* The row keeps its hover while the pointer is on its star (a sibling of the row). */
  .job:hover :global(.row:not(.selected, :active)) {
    background-color: var(--surface-hover);
  }

  .job:hover :global(.row.selected) {
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

  /* Without a ring the dot sits on the axis of the title line. */
  .ringless .dot {
    top: calc(var(--space-12) + (var(--leading-title) - var(--dot-unread)) / 2);
  }

  .title {
    overflow: hidden;
    color: var(--text);
    font: var(--type-title);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title.unread {
    font-weight: var(--weight-semibold);
  }

  .gutter {
    width: 0;
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

  /* The relative date on the title line, right-aligned in the trailing slot; it steps up
     from subtle to muted on hover. */
  .date {
    color: var(--text-subtle);
    font: var(--type-xs);
    line-height: var(--leading-title);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
    transition: color var(--dur-base) var(--ease-standard);
  }

  .job:hover .date {
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

  .reason {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
  }

  /* Room under the date for the tools, so they never cover the meta line. */
  .tool-slot {
    width: var(--control-sm);
    height: var(--control-sm);
  }

  .tool-slot.two {
    width: calc(2 * var(--control-sm) + var(--space-2));
  }

  /* An old date sits on a quiet tint (older than ten days). */
  .date.old {
    padding: 0 var(--space-6);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text-muted);
  }

  .star {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    color: var(--pressed);
  }

  /* The tools over the reserved slot below the date: they fade in on hover (100 ms); a
     pinned star always shows. */
  .tools {
    position: absolute;
    top: calc(var(--space-12) + var(--leading-title) + var(--space-4));
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

  .job:hover .tool,
  .tool:focus-within,
  .pinned .pin {
    opacity: 1;
  }

  .muted:hover .tool,
  .muted .tool:focus-within,
  .muted.pinned .pin {
    opacity: var(--opacity-muted);
  }

  /* No hover while the list scrolls (input.ts). */
  :global(:root[data-scrolling]) .tools {
    pointer-events: none;
  }
</style>
