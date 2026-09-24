<!--
  One job in the list, mail-style with fixed gutters: the unread dot in its own gutter left
  of the ring (so a title never moves when the job is read), the ring, then the title on one
  line with the relative date at its end, company and place, and one reason line with a
  status badge right after it only when something deviates. Every row has the same height.
  The star to pin sits below the date: filled when pinned, otherwise it shows on hover (a
  sibling of the row button, so it never selects the row).
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { displayTitle, formatRelative } from '$lib/i18n/format';
  import { rowReason } from '$lib/i18n/texts';
  import type { JobView } from '$lib/ipc/types';
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
    /** Arrived during the current run. */
    fresh?: boolean;
    /** Fixed "now" for relative dates (gallery and tests). */
    now?: Date;
    onselect?: ((job: JobView) => void) | null;
    /** Pin or unpin from the row; without it a pinned job only shows the star. */
    onpin?: ((job: JobView) => void) | null;
  }

  let {
    job,
    selected = false,
    pending = false,
    ring = true,
    fresh = false,
    now,
    onselect = null,
    onpin = null,
  }: Props = $props();

  const excluded = $derived(job.match?.status === 'excluded');
  const reason = $derived(rowReason(job));
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
  <ScoreRing ring={ringState(job.match, pending)} size="sm" />
{/snippet}

{#snippet endCell()}
  <span class="date">{formatRelative(job.mailDate ?? job.firstSeenAt, now, true)}</span>
  {#if onpin}
    <span class="star-slot" aria-hidden="true"></span>
  {:else if job.pinned}
    <span class="star" role="img" aria-label={de.job.pinned}
      ><Icon name="star" size="sm" filled /></span
    >
  {/if}
{/snippet}

<div class="job" class:pinned={job.pinned}>
  <ListRow
    leading={ring ? ringCell : null}
    trailing={endCell}
    {selected}
    muted={excluded}
    tint={fresh}
    onclick={onselect ? () => onselect?.(job) : null}
    testid="job-row-{job.key.portal}-{job.key.id}"
  >
    <span class="title" class:unread={job.unread}>{heading}</span>
    <span class="meta">
      {#if job.company}<span class="text company">{job.company}</span>{/if}
      {#if job.location}<span class="text place">{job.location}</span>{/if}
    </span>
    <span class="foot">
      {#if reason}
        <span class="reason"><ReasonItem kind={reason.kind} label={reason.text} compact /></span>
      {/if}
      {#if deviation}<Badge label={deviation.label} tone={deviation.tone} />{/if}
    </span>
  </ListRow>
  {#if job.unread}<span class="dot" role="img" aria-label={de.job.unread}></span>{/if}
  {#if onpin}
    <span class="pin">
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
</div>

<style>
  .job {
    position: relative;
  }

  /* The unread dot in its own gutter left of the ring, on the axis of the title line. */
  .dot {
    position: absolute;
    top: calc(var(--space-12) + (var(--leading-title) - var(--dot)) / 2);
    left: calc(var(--pane-padding) - var(--dot-gutter) + (var(--dot-gutter) - var(--dot)) / 2);
    width: var(--dot);
    height: var(--dot);
    border-radius: var(--radius-full);
    background-color: var(--accent);
    pointer-events: none;
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

  /* The relative date on the title line, right-aligned in the trailing slot. */
  .date {
    color: var(--text-subtle);
    font: var(--type-xs);
    line-height: var(--leading-title);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
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

  .star-slot {
    width: var(--control-sm);
    height: var(--control-sm);
  }

  .star {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    color: var(--pressed);
  }

  /* The pin button over the reserved slot below the date. */
  .pin {
    position: absolute;
    top: calc(var(--space-12) + var(--leading-title) + var(--space-4));
    right: var(--pane-padding);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  .job:hover .pin,
  .pin:focus-within,
  .pinned .pin {
    opacity: 1;
  }
</style>
