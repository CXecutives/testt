<!--
  The reader card (max 720 px): ring 96 counting up, band word, must requirements met, the
  hard-criteria strip; for an excluded job the reason first. Then title, company, place,
  portal, date and the actions. "Warum" lists what is met and what is open (must before
  nice), what to check and the violations; hovering a reason lights its passage in the ad
  text below, a click scrolls to it.
-->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Icon from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import ReasonItem from '$components/ReasonItem.svelte';
  import ScoreRing, { ringState } from '$components/ScoreRing.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import { formatDate } from '$lib/i18n/format';
  import {
    criterionKey,
    criterionState,
    errorText,
    noteText,
    reasonHint,
    reasonText,
  } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { JobDetail, OpenTarget, Reason } from '$lib/ipc/types';
  import { isReducedMotion } from '$lib/motion/motion';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import AdText from './AdText.svelte';

  interface Props {
    detail: JobDetail;
  }
  let { detail }: Props = $props();

  const job = $derived(detail.job);
  const match = $derived(detail.match);
  const withRing = $derived(app.hasProfile);
  let active = $state<string | null>(null);
  let textElement = $state<HTMLElement | null>(null);
  let actionError = $state<string | null>(null);

  const WEIGHT_ORDER = { must: 0, hard: 1, nice: 2, info: 3 } as const;
  const byWeight = (a: Reason, b: Reason): number =>
    WEIGHT_ORDER[a.weight] - WEIGHT_ORDER[b.weight];

  const reasons = $derived(match?.reasons ?? []);
  const met = $derived(
    reasons.filter((r) => r.kind === 'met' || r.kind === 'partial').sort(byWeight),
  );
  const open = $derived(reasons.filter((r) => r.kind === 'open').sort(byWeight));
  const checks = $derived(reasons.filter((r) => r.kind === 'check'));
  const allViolations = $derived(reasons.filter((r) => r.kind === 'violation'));

  const STATE_ICON = { met: 'check', violated: 'ban', unknown: 'info', unset: 'minus' } as const;
  const strip = $derived(
    (match?.criteria ?? []).flatMap((reason) => {
      const key = criterionKey(reason.code);
      if (key === null) return [];
      const state = criterionState(reason);
      return [{ id: reason.id, label: de.reader.criterion[key].label, state }];
    }),
  );

  const headline = $derived.by((): { word: string; tone: string } | null => {
    if (!withRing) return null;
    if (match === null) {
      return { word: app.state?.matchPending ? de.score.pending : de.score.none, tone: 'none' };
    }
    if (match.status === 'excluded') return { word: de.score.excluded, tone: 'excluded' };
    if (match.status === 'unscorable') return { word: de.score.unscorable, tone: 'none' };
    return { word: de.score.band[match.band], tone: match.band };
  });
  const exclusion = $derived(
    match?.status === 'excluded'
      ? (noteText(job.match?.note ?? match.summary) ??
          (allViolations[0] ? reasonText(allViolations[0]) : de.reader.note.hardCriterion))
      : null,
  );
  // A violation that says exactly what the notice on top says is not repeated.
  const violations = $derived(allViolations.filter((r) => reasonText(r) !== exclusion));

  const portalState = $derived(app.state?.portals.find((p) => p.portal === job.portal) ?? null);
  const detailKind = $derived(job.detail.kind);
  const canFetch = $derived(
    (detailKind === 'pending' || detailKind === 'failed' || detailKind === 'teaser') &&
      portalState?.enabled === true &&
      portalState.fetchDetails,
  );

  function openTarget(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  function scrollTo(reason: Reason): void {
    active = reason.id;
    const mark = textElement?.querySelector(`[data-reason="${CSS.escape(reason.id)}"]`);
    mark?.scrollIntoView({ block: 'center', behavior: isReducedMotion() ? 'auto' : 'smooth' });
  }

  function hover(reason: Reason, on: boolean): void {
    if (on) active = reason.id;
    else if (active === reason.id) active = null;
  }
</script>

{#snippet reasonList(items: Reason[], testid: string)}
  <ul class="reasons" data-testid={testid}>
    {#each items as reason (reason.id)}
      <li>
        <ReasonItem
          kind={reason.kind}
          weight={reason.weight === 'info' ? null : reason.weight}
          label={reasonText(reason)}
          hint={reasonHint(reason)}
          active={active === reason.id}
          onhover={(on) => hover(reason, on)}
          onselect={reason.ranges.length > 0 ? () => scrollTo(reason) : null}
        />
      </li>
    {/each}
  </ul>
{/snippet}

<Card padding="lg" testid="reader">
  <article class="reader">
    {#if exclusion}
      <Notice tone="danger" text={exclusion} testid="exclusion" />
    {/if}

    {#if headline}
      <header class="score">
        {#key keyOf(job.key)}
          <ScoreRing
            ring={ringState(job.match, job.match === null && Boolean(app.state?.matchPending))}
            size="lg"
            testid="reader-ring"
          />
        {/key}
        <div class="verdict">
          <p class="band {headline.tone}" data-testid="band">{headline.word}</p>
          {#if match && match.status !== 'unscorable'}
            <p class="must" data-testid="must">
              {job.match && job.match.mustTotal > 0
                ? de.reader.mustMet(job.match.mustMet, job.match.mustTotal)
                : de.reader.noMust}
            </p>
          {/if}
          {#if strip.length > 0}
            <ul class="strip" aria-label={de.reader.criteria} data-testid="criteria">
              {#each strip as item (item.id)}
                <li
                  class="criterion {item.state}"
                  use:tooltip={de.reader.criterionState[item.state]}
                  data-state={item.state}
                >
                  <Icon name={STATE_ICON[item.state]} size="sm" />
                  <span>{item.label}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </header>
    {/if}

    <div class="title-block">
      <h1 class="title" data-testid="reader-title">{job.title || de.job.untitled}</h1>
      <p class="meta">
        {#if job.company}<span class="fact"><Icon name="building-2" size="sm" />{job.company}</span
          >{/if}
        {#if job.location}<span class="fact"><Icon name="map-pin" size="sm" />{job.location}</span
          >{/if}
        <span class="fact">{de.portal[job.portal]}</span>
        <span class="fact"
          ><Icon name="clock" size="sm" />{formatDate(job.mailDate ?? job.firstSeenAt)}</span
        >
        {#if job.workMode}<Badge label={de.job.workMode[job.workMode]} />{/if}
      </p>
    </div>

    <div class="actions">
      <Button
        variant="secondary"
        icon="external-link"
        label={de.reader.open}
        testid="open-ad"
        onclick={() => openTarget({ kind: 'jobUrl', key: job.key })}
      />
      <Button
        variant="ghost"
        icon="star"
        label={de.reader.pin}
        pressed={job.pinned}
        testid="pin"
        onclick={() => void jobs.pin(job.key, !job.pinned)}
      />
      {#if detail.mail.gmailUrl}
        <Button
          variant="ghost"
          icon="mail"
          label={de.reader.mail}
          testid="open-mail"
          onclick={() => openTarget({ kind: 'gmail', key: job.key })}
        />
      {/if}
      {#if canFetch}
        <Button
          variant="ghost"
          icon="download"
          label={de.reader.fetchDetails}
          disabled={run.active}
          disabledReason={de.settings.running}
          testid="fetch-details"
          onclick={() => void run.start({ kind: 'details', keys: [job.key] })}
        />
      {/if}
    </div>
    {#if actionError}
      <Notice tone="danger" variant="inline" text={actionError} />
    {/if}

    {#if match && match.status !== 'unscorable' && withRing}
      <section class="why" data-testid="why">
        <h2 class="section">{de.reader.why}</h2>
        {#if met.length + open.length === 0}
          <p class="quiet">{de.reader.noReasons}</p>
        {:else}
          <div class="columns">
            <div class="column">
              <h3 class="sub">{de.reader.met}</h3>
              {@render reasonList(met, 'reasons-met')}
            </div>
            <div class="column">
              <h3 class="sub">{de.reader.missing}</h3>
              {@render reasonList(open, 'reasons-open')}
            </div>
          </div>
        {/if}
        {#if checks.length > 0}
          <h3 class="sub">{de.reader.check}</h3>
          {@render reasonList(checks, 'reasons-check')}
        {/if}
        {#if violations.length > 0}
          <h3 class="sub">{de.reader.violations}</h3>
          {@render reasonList(violations, 'reasons-violation')}
        {/if}
      </section>
    {/if}

    <section class="ad">
      <h2 class="section">{de.reader.ad}</h2>
      {#if detailKind !== 'ok'}
        <Notice
          tone={detailKind === 'gone' || detailKind === 'failed' ? 'warning' : 'info'}
          variant="inline"
          text={portalState && !portalState.fetchDetails && detailKind === 'pending'
            ? de.reader.detailsOff
            : de.reader.detail[detailKind]}
          testid="detail-note"
        />
      {:else if job.short}
        <Notice tone="info" variant="inline" text={de.reader.short} />
      {/if}
      {#if detail.text}
        <AdText
          text={detail.text}
          highlights={match?.highlights ?? []}
          {active}
          bind:element={textElement}
        />
      {/if}
    </section>
  </article>
</Card>

<style>
  .reader {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
  }

  .score {
    display: flex;
    align-items: center;
    gap: var(--space-24);
  }

  .verdict {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }

  .band {
    font: var(--type-xl);
  }

  .band.high {
    color: var(--score-high-text);
  }

  .band.mid {
    color: var(--score-mid-text);
  }

  .band.low,
  .band.none {
    color: var(--score-low-text);
  }

  .band.excluded {
    color: var(--danger-strong);
  }

  .must {
    color: var(--text-muted);
    font: var(--type-md);
    font-variant-numeric: var(--numeric);
  }

  .strip {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    margin-top: var(--space-4);
  }

  .criterion {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    height: var(--badge-height);
    padding: 0 var(--space-8);
    border-radius: var(--radius-full);
    font: var(--type-xs);
    font-weight: var(--weight-semibold);
  }

  .criterion.met {
    background-color: var(--success-soft);
    color: var(--success-strong);
  }

  .criterion.violated {
    background-color: var(--danger-soft);
    color: var(--danger-strong);
  }

  .criterion.unknown {
    background-color: var(--warning-soft);
    color: var(--warning-strong);
  }

  .criterion.unset {
    background-color: var(--surface-muted);
    color: var(--text-subtle);
  }

  .title-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .title {
    color: var(--text-heading);
    font: var(--type-2xl);
    letter-spacing: var(--tracking-tight);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8) var(--space-16);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .fact {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  .why,
  .ad {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding-top: var(--space-24);
    border-top: var(--border-width) solid var(--border);
  }

  .section {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .sub {
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-semibold);
  }

  .columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-24);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: 0;
  }

  .reasons {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
  }

  @media (width < 900px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
