<!--
  The reader, unboxed on the sheet (max 720 px), headed like an issue tracker: the title
  (without gender tags), one line of facts, then one quiet match line (ring 56 counting up,
  the band word and the must requirements met, or for an excluded job the reason) with a
  strip of quiet chips for the contract type and the hard criteria (state by icon only), and
  the actions in one row. "Warum" lists what is met, what is met only in part and what is
  open (must before nice, only "Kann" carries a badge), what to check and what excludes;
  hovering a reason lights its passage in the ad text below, a click scrolls to it. Title,
  facts and the ad text are selectable and copy with Ctrl/Cmd+C (`data-copy`).
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Icon, { type IconName } from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import ReasonItem from '$components/ReasonItem.svelte';
  import ScoreRing, { ringState } from '$components/ScoreRing.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { de, type CriterionState } from '$lib/i18n/de';
  import { displayTitle, formatDate } from '$lib/i18n/format';
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
  // The passage under the pointer wins; a clicked reason keeps its passage marked after the
  // scroll moved the list away from under the pointer.
  let hovered = $state<string | null>(null);
  let pinned = $state<string | null>(null);
  const active = $derived(hovered ?? pinned);
  // Another job starts without a marked passage.
  const shownKey = $derived(keyOf(job.key));
  $effect(() => {
    void shownKey;
    pinned = null;
    hovered = null;
  });
  let textElement = $state<HTMLElement | null>(null);
  let actionError = $state<string | null>(null);

  const WEIGHT_ORDER = { must: 0, hard: 1, nice: 2, info: 3 } as const;
  const byWeight = (a: Reason, b: Reason): number =>
    WEIGHT_ORDER[a.weight] - WEIGHT_ORDER[b.weight];

  const CONTRACT = 'contractType';
  const all = $derived(match?.reasons ?? []);
  // The contract type is a fact about the ad, not a requirement: it goes into the chips.
  const contract = $derived(all.find((r) => r.code === CONTRACT) ?? null);
  const reasons = $derived(all.filter((r) => r.code !== CONTRACT));
  const met = $derived(reasons.filter((r) => r.kind === 'met').sort(byWeight));
  // Met only in part is not met: its own group, never under "Erfüllt".
  const partial = $derived(reasons.filter((r) => r.kind === 'partial').sort(byWeight));
  const open = $derived(reasons.filter((r) => r.kind === 'open').sort(byWeight));
  const checks = $derived(reasons.filter((r) => r.kind === 'check'));
  const allViolations = $derived(reasons.filter((r) => r.kind === 'violation'));
  const partialMust = $derived(
    reasons.filter((r) => r.kind === 'partial' && r.weight === 'must').length,
  );

  const STATE_ICON: Record<CriterionState, IconName> = {
    met: 'check',
    violated: 'x',
    unknown: 'circle-help',
    unset: 'minus',
  };
  interface Chip {
    id: string;
    label: string;
    state: CriterionState | 'plain';
    icon: IconName;
    hint: string;
  }
  const chips = $derived.by((): Chip[] => {
    const out: Chip[] = [];
    // The contract chip steps back when a criterion chip carries the same word (ANÜ).
    const labels = new Set(
      (match?.criteria ?? []).flatMap((reason) => {
        const key = criterionKey(reason.code);
        return key === null ? [] : [de.reader.criterion[key].label];
      }),
    );
    if (contract && !labels.has(reasonText(contract))) {
      const unclear = contract.kind === 'check';
      out.push({
        id: contract.id,
        label: reasonText(contract),
        state: unclear ? 'unknown' : 'plain',
        icon: unclear ? 'circle-help' : 'file-text',
        hint: de.reader.contractLabel,
      });
    }
    for (const reason of match?.criteria ?? []) {
      const key = criterionKey(reason.code);
      if (key === null) continue;
      const state = criterionState(reason);
      out.push({
        id: reason.id,
        label: de.reader.criterion[key].label,
        state,
        icon: STATE_ICON[state],
        hint: de.reader.criterionState[state],
      });
    }
    return out;
  });

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
  // A violation that says exactly what the match line says is not repeated.
  const violations = $derived(allViolations.filter((r) => reasonText(r) !== exclusion));

  const portalState = $derived(app.state?.portals.find((p) => p.portal === job.portal) ?? null);
  const detailKind = $derived(job.detail.kind);
  const canFetch = $derived(
    (detailKind === 'pending' || detailKind === 'failed' || detailKind === 'teaser') &&
      portalState?.enabled === true &&
      portalState.fetchDetails,
  );
  const facts = $derived(
    [
      job.company,
      job.location,
      job.workMode ? de.job.workMode[job.workMode] : '',
      job.alsoOn.length > 0
        ? `${de.portal[job.portal]}, ${de.job.alsoOn(job.alsoOn.map((p) => de.portal[p]).join(', '))}`
        : de.portal[job.portal],
      formatDate(job.mailDate ?? job.firstSeenAt),
    ].filter((fact) => fact !== ''),
  );

  function openTarget(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  function scrollTo(reason: Reason): void {
    pinned = reason.id;
    const mark = textElement?.querySelector(`[data-reason="${CSS.escape(reason.id)}"]`);
    mark?.scrollIntoView({ block: 'center', behavior: isReducedMotion() ? 'auto' : 'smooth' });
  }

  function hover(reason: Reason, on: boolean): void {
    if (on) hovered = reason.id;
    else if (hovered === reason.id) hovered = null;
  }
</script>

{#snippet reasonList(items: Reason[], testid: string)}
  <ul class="reasons" data-testid={testid}>
    {#each items as reason (reason.id)}
      <li data-weight={reason.weight}>
        <ReasonItem
          kind={reason.kind}
          weight={reason.weight === 'nice' ? 'nice' : null}
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

<article class="reader" data-testid="reader">
  <header class="head">
    <h1 class="title" data-testid="reader-title" data-copy>
      {job.title ? displayTitle(job.title) : de.job.untitled}
    </h1>
    <p class="facts" data-copy>
      <span class="facts-line">
        {#each facts as fact, index (index)}<span class="fact">{fact}</span>{/each}
      </span>
    </p>
  </header>

  {#if headline}
    <div class="match">
      {#key keyOf(job.key)}
        <ScoreRing
          ring={ringState(job.match, job.match === null && Boolean(app.state?.matchPending))}
          size="md"
          animate={keyOf(job.key)}
          testid="reader-ring"
        />
      {/key}
      <div class="verdict">
        <p class="line">
          <span class="band {headline.tone}" data-testid="band">{headline.word}</span>
          {#if match && match.status === 'scored'}
            <span class="must" data-testid="must">
              {job.match && job.match.mustTotal > 0
                ? de.reader.mustMet(job.match.mustMet, job.match.mustTotal, partialMust)
                : de.reader.noMust}
            </span>
          {/if}
        </p>
        {#if exclusion}
          <p class="because" data-testid="exclusion">{exclusion}</p>
        {/if}
        {#if chips.length > 0}
          <ul class="chips" aria-label={de.reader.criteria} data-testid="criteria">
            {#each chips as chip (chip.id)}
              <li
                class="chip {chip.state}"
                use:tooltip={chip.hint}
                data-state={chip.state}
                data-testid={chip.id === contract?.id ? 'contract' : undefined}
              >
                <span class="chip-icon"><Icon name={chip.icon} size="xs" /></span>
                <span>{chip.label}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}

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
      {#if met.length + partial.length + open.length === 0}
        <p class="quiet">{de.reader.noReasons}</p>
      {:else}
        <div class="columns">
          {#if met.length + partial.length > 0}
            <div class="stack">
              {#if met.length > 0}
                <div class="group">
                  <h3 class="sub">{de.reader.met}</h3>
                  {@render reasonList(met, 'reasons-met')}
                </div>
              {/if}
              {#if partial.length > 0}
                <div class="group">
                  <h3 class="sub">{de.reader.partial}</h3>
                  {@render reasonList(partial, 'reasons-partial')}
                </div>
              {/if}
            </div>
          {/if}
          {#if open.length > 0}
            <div class="group">
              <h3 class="sub">{de.reader.missing}</h3>
              {@render reasonList(open, 'reasons-open')}
            </div>
          {/if}
        </div>
      {/if}
      {#if checks.length > 0}
        <div class="group">
          <h3 class="sub">{de.reader.check}</h3>
          {@render reasonList(checks, 'reasons-check')}
        </div>
      {/if}
      {#if violations.length > 0}
        <div class="group">
          <h3 class="sub">{de.reader.violations}</h3>
          {@render reasonList(violations, 'reasons-violation')}
        </div>
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

<style>
  .reader {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    container-type: inline-size;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .title {
    color: var(--text-heading);
    font: var(--type-2xl);
    letter-spacing: var(--tracking-tight);
    text-wrap: balance;
  }

  /* Facts joined by middle dots; a dot that would start a wrapped line is clipped (every
     fact carries its dot in front, the line is shifted left by one dot). */
  .facts {
    overflow: hidden;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .facts-line {
    display: flex;
    flex-wrap: wrap;
    margin-left: calc(-1 * var(--space-20));
  }

  .fact::before {
    display: inline-block;
    width: var(--space-20);
    color: var(--text-subtle);
    text-align: center;
    content: '·';
  }

  /* One quiet match line: the ring, the band word with the must count, the chips. */
  .match {
    display: flex;
    align-items: center;
    gap: var(--space-16);
  }

  .verdict {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    font: var(--type-md);
  }

  .band {
    font-weight: var(--weight-medium);
  }

  .must {
    color: var(--text-muted);
  }

  .must::before {
    padding: 0 var(--space-6);
    color: var(--text-subtle);
    content: '·';
  }

  .because {
    color: var(--text-muted);
    font: var(--type-sm);
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

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
  }

  /* Quiet chips: a neutral name, the state only in the icon (and in red when violated). */
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    height: var(--badge-height);
    padding: 0 var(--space-8) 0 var(--space-6);
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
    color: var(--text-muted);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }

  .chip-icon {
    display: inline-flex;
    color: var(--chip-icon, var(--text-subtle));
  }

  .chip.met {
    --chip-icon: var(--success-strong);
  }

  .chip.unknown {
    --chip-icon: var(--warning-strong);
  }

  .chip.violated {
    --chip-icon: var(--danger-strong);

    background-color: var(--danger-soft);
    color: var(--danger-strong);
  }

  .chip.unset {
    color: var(--text-muted);
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
    gap: var(--space-16);
    padding-top: var(--space-20);
    border-top: var(--border-width) solid var(--border);
  }

  .section {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  /* Sentence case, quiet: the groups of "Warum". */
  .sub {
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  /* One list; two columns only where there is room for them. */
  .columns {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--space-16);
  }

  @container (width >= 720px) {
    .columns {
      grid-template-columns: 1fr 1fr;
      gap: var(--space-24);
    }
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    min-width: 0;
  }

  .reasons {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-left: calc(-1 * var(--space-8));
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
  }
</style>
