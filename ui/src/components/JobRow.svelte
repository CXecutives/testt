<!--
  One job in the list: ring, unread dot, title, meta (portal mark, company, location, work
  mode), one reason line, relative date, a status badge only when something deviates, and
  the star when pinned.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import { formatRelative } from '$lib/i18n/format';
  import { rowReason } from '$lib/i18n/texts';
  import type { JobView } from '$lib/ipc/types';
  import Badge, { type BadgeTone } from './Badge.svelte';
  import Icon from './Icon.svelte';
  import { PORTAL_MONOGRAM } from './IconTile.svelte';
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
  }

  let {
    job,
    selected = false,
    pending = false,
    ring = true,
    fresh = false,
    now,
    onselect = null,
  }: Props = $props();

  const excluded = $derived(job.match?.status === 'excluded');
  const reason = $derived(rowReason(job));

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

  const alsoOn = $derived(
    job.alsoOn.length > 0 ? de.job.alsoOn(job.alsoOn.map((p) => de.portal[p]).join(', ')) : null,
  );
</script>

{#snippet ringCell()}
  <ScoreRing ring={ringState(job.match, pending)} size="sm" />
{/snippet}

<ListRow
  leading={ring ? ringCell : null}
  {selected}
  muted={excluded}
  tint={fresh}
  onclick={onselect ? () => onselect?.(job) : null}
  testid="job-row-{job.key.portal}-{job.key.id}"
>
  <span class="title-line">
    {#if job.unread}<span class="dot" role="img" aria-label={de.job.unread}></span>{/if}
    <span class="title" class:unread={job.unread}>{job.title || de.job.untitled}</span>
  </span>
  <span class="meta">
    <span
      class="portal"
      role="img"
      aria-label={de.portal[job.portal]}
      use:tooltip={de.portal[job.portal]}>{PORTAL_MONOGRAM[job.portal]}</span
    >
    {#if alsoOn}<span class="also" use:tooltip={alsoOn}>+{job.alsoOn.length}</span>{/if}
    <span class="text company">{job.company}</span>
    {#if job.location}<span class="sep">·</span><span class="text place">{job.location}</span>{/if}
    {#if job.workMode}<span class="mode"
        ><Badge label={de.job.workMode[job.workMode]} tone="neutral" /></span
      >{/if}
  </span>
  {#if reason}
    <span class="reason"><ReasonItem kind={reason.kind} label={reason.text} compact /></span>
  {/if}

  {#snippet trailing()}
    <span class="date">{formatRelative(job.mailDate ?? job.firstSeenAt, now)}</span>
    <span class="flags">
      {#if deviation}<Badge label={deviation.label} tone={deviation.tone} />{/if}
      {#if job.pinned}
        <span class="star" role="img" aria-label={de.job.pinned}
          ><Icon name="star" size="sm" filled /></span
        >
      {/if}
    </span>
  {/snippet}
</ListRow>

<style>
  .title-line {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
  }

  .dot {
    flex: none;
    width: var(--dot);
    height: var(--dot);
    border-radius: var(--radius-full);
    background-color: var(--accent);
  }

  .title {
    overflow: hidden;
    color: var(--text);
    font: var(--type-md);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title.unread {
    font-weight: var(--weight-semibold);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .portal {
    flex: none;
    padding: 0 var(--space-4);
    border-radius: var(--radius-xs);
    background-color: var(--surface-muted);
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-bold);
  }

  .also {
    flex: none;
    color: var(--text-subtle);
    font: var(--type-xs);
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .company {
    flex: 0 1 auto;
    min-width: var(--space-48);
  }

  .sep {
    color: var(--text-subtle);
  }

  /* The place is short and says more than the end of a long company name. */
  .place {
    flex: 0 1 auto;
    max-width: 45%;
  }

  /* In a narrow list the work mode gives way to company and place (the reader shows it). */
  @container (width < 460px) {
    .mode {
      display: none;
    }
  }

  .reason {
    display: flex;
    min-width: 0;
  }

  .date {
    color: var(--text-subtle);
    font: var(--type-xs);
    font-variant-numeric: var(--numeric);
    white-space: nowrap;
  }

  .flags {
    display: flex;
    align-items: center;
    gap: var(--space-6);
  }

  .star {
    display: inline-flex;
    color: var(--accent);
  }
</style>
