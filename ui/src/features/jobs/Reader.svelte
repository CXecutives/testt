<!--
  The reader, unboxed on the sheet (max 720 px), headed like an issue tracker: the title
  (without gender tags), one line of facts, then one quiet match line (ring 56 counting up,
  the band word and the must requirements met, or for an excluded job the reason) with a
  strip of quiet chips for the contract type and the hard criteria (state by icon only), and
  the actions in one row. "Warum" lists what is met, what is met only in part and what is
  open (must before nice, only "Kann" carries a badge), what to check and what excludes;
  hovering a reason lights its passage in the ad text below, a click scrolls to it. Title,
  facts and the ad text are selectable and copy with Ctrl/Cmd+C (`data-copy`).
  Actions by weight: at the end of the title line the star (Merken), Archivieren and a quiet
  close back to the day overview (below 900 px the view's back button does); below the match
  line "Anzeige öffnen" first, then "Als Prompt kopieren" (the job as a prompt for any AI
  chat), the alert mail and "Details holen" when the details are missing (right under the
  band when the job has no score yet). Then the user's own marks: where the application
  stands (one chip per step, the chosen one again clears it, with the time it was set) and
  a note that saves when the field is left (Enter saves, Esc takes the stored one back).
  After Archivieren the next job of the list opens, and the toast can take it back. The groups of "Warum" carry navy sub-labels with a soft count; a reason
  that jumps to its passage makes the passage flash once when it has arrived. Once the
  action row has scrolled away, a compact bar sticks to the top (ring, title, open, pin):
  it fades in sliding down 4 px and leaves faster, and it cannot be clicked while hidden.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import Chip from '$components/Chip.svelte';
  import Count from '$components/Count.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Icon, { type IconName } from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import TextField from '$components/TextField.svelte';
  import ReasonItem from '$components/ReasonItem.svelte';
  import ScoreRing, { ringState } from '$components/ScoreRing.svelte';
  import { inView, scrollArea } from '$lib/actions/inView';
  import { tooltip } from '$lib/actions/tooltip';
  import { de, type CriterionState } from '$lib/i18n/de';
  import { displayTitle, formatDate, formatRelative } from '$lib/i18n/format';
  import {
    criterionKey,
    criterionState,
    criterionValue,
    errorText,
    noteText,
    reasonEvidence,
    reasonHint,
    reasonText,
  } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import { formKeys } from '$lib/input/input';
  import type { AppStatus, JobDetail, OpenTarget, Reason } from '$lib/ipc/types';
  import { duration, isReducedMotion } from '$lib/motion/motion';
  import { app } from '$lib/state/app.svelte';
  import { isApplication, jobs, keyOf } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import AdText from './AdText.svelte';
  import { archive } from './archive';

  interface Props {
    detail: JobDetail;
    /** Close the job (back to the day overview). */
    onclose?: (() => void) | null;
  }
  let { detail, onclose = null }: Props = $props();

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
  /** The action row has scrolled away: the compact bar is up. */
  let compact = $state(false);
  /** The passage that flashes once after a jump to it. */
  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  const WEIGHT_ORDER = { must: 0, hard: 1, nice: 2, info: 3 } as const;
  const byWeight = (a: Reason, b: Reason): number =>
    WEIGHT_ORDER[a.weight] - WEIGHT_ORDER[b.weight];

  const CONTRACT = 'contractType';
  const all = $derived(match?.reasons ?? []);
  // The contract type is a fact about the ad, not a requirement: it goes into the chips.
  const contract = $derived(all.find((r) => r.code === CONTRACT) ?? null);
  // Wishes of the profile move a score a little and never exclude: their own block.
  const WISHES: readonly string[] = ['dayRateWish', 'remoteWish', 'regionWish', 'industryWish'];
  const wishes = $derived(all.filter((r) => WISHES.includes(r.code)));
  const reasons = $derived(all.filter((r) => r.code !== CONTRACT && !WISHES.includes(r.code)));
  // Only passages a reason under "Warum" explains are marked (the contract type is a chip).
  const passages = $derived([
    ...(match?.highlights ?? []).filter((h) => contract === null || h.reason !== contract.id),
    ...(match?.criteria ?? []).flatMap((reason) =>
      reason.ranges.map((range, index) => ({
        id: `${reason.id}:${index}`,
        start: range.start,
        end: range.end,
        kind: reason.kind,
        reason: reason.id,
      })),
    ),
  ]);
  const met = $derived(reasons.filter((r) => r.kind === 'met').sort(byWeight));
  // Met only in part is not met: its own group, never under "Erfüllt".
  const partial = $derived(reasons.filter((r) => r.kind === 'partial').sort(byWeight));
  const open = $derived(reasons.filter((r) => r.kind === 'open').sort(byWeight));
  // Under "Zu prüfen" what decides fastest comes first: the temporary agency work (ANÜ).
  const FIRST_CHECKS: readonly string[] = ['anue'];
  const checks = $derived(
    reasons
      .filter((r) => r.kind === 'check')
      .sort(
        (a, b) => Number(FIRST_CHECKS.includes(b.code)) - Number(FIRST_CHECKS.includes(a.code)),
      ),
  );
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
  interface StripChip {
    id: string;
    label: string;
    state: CriterionState | 'plain';
    icon: IconName;
    hint: string;
    reason: Reason | null;
  }
  const chips = $derived.by((): StripChip[] => {
    const out: StripChip[] = [];
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
        reason: contract.ranges.length > 0 ? contract : null,
      });
    }
    for (const reason of match?.criteria ?? []) {
      const key = criterionKey(reason.code);
      if (key === null) continue;
      const state = criterionState(reason);
      const name = de.reader.criterion[key].label;
      const value = criterionValue(reason);
      out.push({
        id: reason.id,
        // The ad's own value; what it does not mention says so, neutral.
        label: value ?? (state === 'unset' ? de.facts.notMentioned(name) : name),
        state,
        icon: STATE_ICON[state],
        hint:
          key === 'noAnue' && state === 'unknown'
            ? de.reader.anueCheck
            : value
              ? `${name}, ${de.reader.criterionState[state]}`
              : de.reader.criterionState[state],
        reason: reason.ranges.length > 0 ? reason : null,
      });
    }
    return out;
  });

  // Every criterion met with the ad as evidence: one quiet line of the values, no chips.
  const clean = $derived(
    chips.length > 0 && chips.every((c) => c.state === 'met' || c.state === 'plain'),
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
  // Why a job cannot be scored (too little text, an engine failure); without a note the ad
  // simply names no clear requirements.
  const unscorable = $derived(
    match?.status === 'unscorable'
      ? (noteText(job.match?.note ?? match.summary) ?? de.reader.noReasons)
      : null,
  );
  // A violation that says exactly what the match line says is not repeated.
  const violations = $derived(allViolations.filter((r) => reasonText(r) !== exclusion));

  const portalState = $derived(app.state?.portals.find((p) => p.portal === job.portal) ?? null);
  const detailKind = $derived(job.detail.kind);
  const canFetch = $derived(
    (detailKind === 'pending' || detailKind === 'failed' || detailKind === 'teaser') &&
      portalState?.enabled === true &&
      portalState.fetchDetails &&
      (detailKind !== 'teaser' || portalState.loginEnabled),
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

  /** No score yet and the details can be fetched: the button stands right under the band. */
  const fetchUnderBand = $derived(
    canFetch && withRing && (match === null || match.status === 'unscorable'),
  );
  /** A score from a teaser only is a first guess. */
  const preliminary = $derived(match?.status === 'scored' && detailKind === 'teaser');

  const STATUSES: readonly AppStatus[] = ['applied', 'interview', 'offer', 'rejected'];
  /** The note as typed; the stored one is the detail's. */
  let note = $state(untrack(() => detail.note ?? ''));

  async function setStatus(status: AppStatus): Promise<void> {
    actionError = null;
    const error = await jobs.setAppStatus(job.key, job.appStatus === status ? null : status);
    if (error !== null) actionError = error;
  }

  async function saveNote(): Promise<void> {
    if (note.trim() === (detail.note ?? '').trim()) return;
    actionError = null;
    const error = await jobs.setNote(job.key, note);
    if (error !== null) actionError = error;
  }

  function revertNote(): void {
    note = detail.note ?? '';
  }

  async function copyPrompt(): Promise<void> {
    actionError = null;
    try {
      await navigator.clipboard.writeText(await jobs.aiPrompt(job.key));
      toasts.show(de.toast.prompt);
    } catch (error) {
      actionError = errorText(error);
    }
  }

  /** Archive the job (the next one opens, the toast takes it back), or bring it back. */
  async function hide(): Promise<void> {
    actionError = await archive(job);
  }

  let confirmDelete = $state(false);
  let deleting = $state(false);
  let deleteError = $state<string | null>(null);

  async function deleteJob(): Promise<void> {
    deleting = true;
    deleteError = null;
    const result = await jobs.deleteJobs([job.key]);
    deleting = false;
    if ('error' in result) {
      deleteError = result.error;
      return;
    }
    confirmDelete = false;
    toasts.show(de.toast.deleted(result.count));
    void jobs.loadOverview();
  }

  /** "Trotzdem passend": an excluded job counts with its fit score, and back. */
  async function override(): Promise<void> {
    actionError = await jobs.setOverride(job.key, !job.overridden);
  }

  function openTarget(target: OpenTarget): void {
    actionError = null;
    invoke('open_target', { target }).catch((error: unknown) => (actionError = errorText(error)));
  }

  function flashPassage(id: string): void {
    if (flashTimer !== null) clearTimeout(flashTimer);
    flash = null;
    requestAnimationFrame(() => {
      flash = id;
      flashTimer = setTimeout(() => (flash = null), duration('base'));
    });
  }

  /** Once the scroll area has come to rest (at the latest after the time a scroll takes). */
  function afterScroll(area: Element | null, then: () => void): void {
    let done = false;
    const finish = (): void => {
      if (done) return;
      done = true;
      area?.removeEventListener('scrollend', finish);
      then();
    };
    area?.addEventListener('scrollend', finish);
    setTimeout(finish, 2 * duration('reveal'));
  }

  function scrollTo(reason: Reason): void {
    pinned = reason.id;
    const mark = textElement?.querySelector(`[data-reason="${CSS.escape(reason.id)}"]`);
    if (!mark) return;
    const area = scrollArea(mark);
    const box = mark.getBoundingClientRect();
    const view = area?.getBoundingClientRect() ?? { top: 0, bottom: innerHeight };
    const inside = box.top >= view.top && box.bottom <= view.bottom;
    if (isReducedMotion()) {
      if (!inside) mark.scrollIntoView({ block: 'center' });
      return;
    }
    if (inside) {
      flashPassage(reason.id);
      return;
    }
    afterScroll(area, () => flashPassage(reason.id));
    mark.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  function hover(reason: Reason, on: boolean): void {
    if (on) hovered = reason.id;
    else if (hovered === reason.id) hovered = null;
  }
</script>

{#snippet sub(label: string, count: number)}
  <h3 class="sub">{label}<Count value={count} /></h3>
{/snippet}

{#snippet reasonList(items: Reason[], testid: string)}
  <ul class="reasons" data-testid={testid}>
    {#each items as reason (reason.id)}
      {@const evidence = reasonEvidence(reason)}
      <li data-weight={reason.weight}>
        <ReasonItem
          kind={reason.kind}
          weight={reason.weight === 'nice'
            ? 'nice'
            : reason.kind === 'open' && reason.weight === 'must'
              ? 'must'
              : null}
          label={reasonText(reason)}
          hint={evidence ? null : reasonHint(reason)}
          active={active === reason.id}
          onhover={(on) => hover(reason, on)}
          onselect={reason.ranges.length > 0 ? () => scrollTo(reason) : null}
        />
        {#if evidence}<p class="evidence" data-testid="evidence">{evidence}</p>{/if}
      </li>
    {/each}
  </ul>
{/snippet}

<article class="reader" data-testid="reader">
  <!-- Sticks to the top of the stage; up only while the action row is scrolled away. -->
  <div class="compact-anchor">
    <div
      class="compact"
      class:shown={compact}
      aria-hidden={!compact}
      inert={!compact}
      data-testid="reader-compact"
    >
      {#if withRing}
        <ScoreRing
          ring={ringState(
            job.match,
            job.match === null && Boolean(app.state?.matchPending),
            job.detail.kind,
          )}
          size="sm"
        />
      {/if}
      <span class="compact-title">{job.title ? displayTitle(job.title) : de.job.untitled}</span>
      <span class="compact-tools">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="external-link"
          label={de.reader.open}
          testid="compact-open"
          onclick={() => openTarget({ kind: 'jobUrl', key: job.key })}
        />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="star"
          label={job.pinned ? de.reader.unpin : de.reader.pin}
          pressed={job.pinned}
          testid="compact-pin"
          onclick={() => void jobs.pin(job.key, !job.pinned)}
        />
      </span>
    </div>
  </div>

  <header class="head">
    <div class="title-line">
      <h1 class="title" data-testid="reader-title" data-copy>
        {job.title ? displayTitle(job.title) : de.job.untitled}
      </h1>
      <span class="title-tools">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="star"
          label={job.pinned ? de.reader.unpin : de.reader.pin}
          pressed={job.pinned}
          testid="pin"
          onclick={() => void jobs.pin(job.key, !job.pinned)}
        />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon={job.archived ? 'archive-restore' : 'archive'}
          label={job.archived ? de.reader.restore : de.reader.archive}
          testid="hide"
          onclick={() => void hide()}
        />
        {#if onclose}
          <span class="close">
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              icon="x"
              label={de.reader.close}
              testid="reader-close"
              onclick={onclose}
            />
          </span>
        {/if}
      </span>
    </div>
    <p class="facts" data-copy>
      <span class="facts-line">
        {#each facts as fact, index (index)}<span class="fact">{fact}</span>{/each}
      </span>
    </p>
  </header>
  {#if job.archived}
    <div class="archived" data-testid="archived-note">
      <Notice
        tone="info"
        variant="row"
        text={de.reader.archived}
        action={{ label: de.reader.restore, onclick: () => void hide() }}
      />
      <Button
        variant="ghost"
        size="sm"
        icon="trash-2"
        label={de.reader.deleteForGood}
        testid="delete-job"
        onclick={() => (confirmDelete = true)}
      />
    </div>
  {/if}

  {#if headline}
    <div class="match">
      <ScoreRing
        ring={ringState(
          job.match,
          job.match === null && Boolean(app.state?.matchPending),
          job.detail.kind,
        )}
        size="md"
        animate={keyOf(job.key)}
        testid="reader-ring"
      />
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
          <p class="because" data-testid="exclusion">
            {exclusion}
            <span class="inline-action">
              <Button
                variant="link"
                size="sm"
                label={de.reader.override}
                testid="override"
                onclick={() => void override()}
              />
            </span>
          </p>
        {:else if job.overridden}
          <p class="because" data-testid="overridden">
            {de.reader.overridden}
            <span class="inline-action">
              <Button
                variant="link"
                size="sm"
                label={de.reader.overrideUndo}
                testid="override-undo"
                onclick={() => void override()}
              />
            </span>
          </p>
        {:else if unscorable}
          <p class="because" data-testid="unscorable">{unscorable}</p>
        {:else if preliminary}
          <p class="because" data-testid="preliminary">{de.reader.preliminary}</p>
        {/if}
        {#if fetchUnderBand}
          <span class="fetch-here">
            <Button
              variant="secondary"
              size="sm"
              icon="download"
              label={de.reader.fetchDetails}
              disabled={run.active}
              disabledReason={run.busyText}
              testid="fetch-details"
              onclick={() => void run.start({ kind: 'details', keys: [job.key] })}
            />
          </span>
        {/if}
        {#if clean}
          <p class="clean" aria-label={de.reader.frame} data-testid="criteria-clean">
            <span class="strip-label">{de.reader.frame}</span>
            <span class="clean-icon"><Icon name="check" size="xs" /></span>
            <span class="clean-values" data-copy
              >{#each chips as chip (chip.id)}<span class="fact">{chip.label}</span>{/each}</span
            >
          </p>
        {:else if chips.length > 0}
          <ul class="chips" aria-label={de.reader.frame} data-testid="criteria">
            <li class="strip-label">{de.reader.frame}</li>
            {#each chips as chip (chip.id)}
              {@const target = chip.reason}
              <li data-testid={chip.id === contract?.id ? 'contract' : `criterion-${chip.id}`}>
                <Chip
                  label={chip.label}
                  state={chip.state}
                  icon={chip.icon}
                  hint={chip.hint}
                  active={active === chip.id}
                  onhover={target ? (on) => hover(target, on) : null}
                  onselect={target ? () => scrollTo(target) : null}
                />
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
    <span class="with-hint" use:tooltip={de.reader.promptHint}>
      <Button
        variant="ghost"
        icon="copy"
        label={de.reader.prompt}
        testid="prompt"
        onclick={() => void copyPrompt()}
      />
    </span>
    {#if detail.mail.gmailUrl}
      <Button
        variant="ghost"
        icon="mail"
        label={de.reader.mail}
        testid="open-mail"
        onclick={() => openTarget({ kind: 'gmail', key: job.key })}
      />
    {/if}
    {#if canFetch && !fetchUnderBand}
      <Button
        variant="ghost"
        icon="download"
        label={de.reader.fetchDetails}
        disabled={run.active}
        disabledReason={run.busyText}
        testid="fetch-details"
        onclick={() => void run.start({ kind: 'details', keys: [job.key] })}
      />
    {/if}
  </div>
  <div class="marks">
    <div class="status" role="group" aria-label={de.reader.status} data-testid="status">
      {#each STATUSES as status (status)}
        <Button
          variant="secondary"
          size="sm"
          label={de.reader.appStatus[status]}
          pressed={job.appStatus === status}
          testid="status-{status}"
          onclick={() => void setStatus(status)}
        />
      {/each}
      {#if isApplication(job.appStatus) && job.statusAt}
        <span class="since" data-testid="status-since">{formatRelative(job.statusAt)}</span>
      {/if}
    </div>
    <span
      class="note"
      onfocusout={() => void saveNote()}
      use:formKeys={{ save: () => void saveNote(), cancel: revertNote }}
    >
      <TextField
        value={note}
        label={de.reader.noteLabel}
        placeholder={de.reader.noteLabel}
        testid="note"
        oninput={(value) => (note = value)}
      />
    </span>
  </div>
  <span class="past-actions" use:inView={(place) => (compact = place === 'above')}></span>
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
                  {@render sub(de.reader.met, met.length)}
                  {@render reasonList(met, 'reasons-met')}
                </div>
              {/if}
              {#if partial.length > 0}
                <div class="group">
                  {@render sub(de.reader.partial, partial.length)}
                  {@render reasonList(partial, 'reasons-partial')}
                </div>
              {/if}
            </div>
          {/if}
          {#if open.length > 0}
            <div class="group">
              {@render sub(de.reader.missing, open.length)}
              {@render reasonList(open, 'reasons-open')}
            </div>
          {/if}
        </div>
      {/if}
      {#if checks.length > 0}
        <div class="group">
          {@render sub(de.reader.check, checks.length)}
          {@render reasonList(checks, 'reasons-check')}
        </div>
      {/if}
      {#if wishes.length > 0}
        <div class="group" data-testid="wishes">
          {@render sub(de.reader.wishes, wishes.length)}
          {@render reasonList(wishes, 'reasons-wish')}
        </div>
      {/if}
      {#if violations.length > 0}
        <div class="group">
          {@render sub(de.reader.violations, violations.length)}
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
        text={portalState &&
        (!portalState.enabled || !portalState.fetchDetails) &&
        detailKind === 'pending'
          ? de.reader.detailsOff
          : de.reader.detail[detailKind]}
        testid="detail-note"
      />
    {:else if job.short && unscorable === null}
      <Notice tone="info" variant="inline" text={de.reader.short} />
    {/if}
    {#if detail.text}
      <AdText
        text={detail.text}
        highlights={passages}
        {active}
        {flash}
        bind:element={textElement}
      />
    {/if}
  </section>
</article>

<Dialog
  bind:open={confirmDelete}
  variant="danger"
  heading={de.reader.deleteHeading}
  text={de.reader.deleteText}
  confirmLabel={de.reader.deleteForGood}
  busy={deleting}
  error={deleteError}
  testid="dialog-delete-job"
  onconfirm={() => void deleteJob()}
/>

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

  /* No room of its own: it sticks to the top of the stage (under the macOS toolbar row)
     and cancels the gap it would add. */
  .compact-anchor {
    position: sticky;
    top: var(--window-top);
    z-index: var(--z-sticky);
    height: 0;
    margin-bottom: calc(-1 * var(--space-20));
  }

  /* The compact bar over the whole column: ring, title, open, pin. Enters in 150 ms with
     ease-out sliding down 4 px, leaves in 100 ms with ease-in. */
  .compact {
    position: absolute;
    top: 0;
    right: calc(-1 * var(--reader-padding));
    left: calc(-1 * var(--reader-padding));
    display: flex;
    align-items: center;
    gap: var(--space-12);
    height: var(--compact-header);
    padding: 0 var(--reader-padding);
    border-bottom: var(--border-width) solid var(--border);
    background-color: var(--surface);
    opacity: 0;
    pointer-events: none;
    transform: translateY(calc(-1 * var(--move-md)));
    transition:
      opacity var(--dur-fast) var(--ease-in),
      transform var(--dur-fast) var(--ease-in);
    will-change: transform;
  }

  .compact.shown {
    opacity: 1;
    pointer-events: auto;
    transform: none;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out);
  }

  .compact-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-title);
    font-weight: var(--weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compact-tools {
    display: flex;
    flex: none;
    gap: var(--space-2);
    margin-right: calc(-1 * var(--space-6));
  }

  /* Watched: once it has scrolled away, the compact bar comes up. No room of its own. */
  .past-actions {
    height: 0;
    margin-top: calc(-1 * var(--space-20));
  }

  .title-line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
  }

  .title {
    flex: 1;
    min-width: 0;
    color: var(--text-heading);
    font: var(--type-2xl);
    letter-spacing: var(--tracking-tight);
    text-wrap: balance;
  }

  /* On the axis of the first title line; the last glyph ends on the edge of the column. */
  .title-tools {
    display: flex;
    flex: none;
    gap: var(--space-2);
    margin-top: calc((var(--leading-2xl) - var(--control-sm)) / 2);
    margin-right: calc(-1 * var(--space-6));
  }

  .close {
    display: flex;
  }

  @media (width < 900px) {
    .close {
      display: none;
    }
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

  /* Every criterion met: the values in one quiet line after a green check. */
  .strip-label {
    display: inline-flex;
    align-items: center;
    color: var(--text-label);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
  }

  .clean {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .clean-icon {
    display: inline-flex;
    color: var(--success-strong);
  }

  .clean-values {
    display: flex;
    flex-wrap: wrap;
    margin-left: calc(-1 * var(--space-20));
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  .inline-action {
    display: inline-flex;
    margin-left: var(--space-6);
    vertical-align: baseline;
  }

  .with-hint {
    display: inline-flex;
  }

  /* An archived job says so under its facts, with the way back and the way out. */
  .archived {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
  }

  .archived > :global(:first-child) {
    flex: 1;
  }

  .fetch-here {
    display: flex;
    margin-top: var(--space-2);
  }

  /* The user's own marks: the step of the application, the note. */
  .marks {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-6);
  }

  .since {
    margin-left: var(--space-2);
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .note {
    display: flex;
    max-width: var(--list-max);
  }

  /* The evidence of a point, quiet under it (on the axis of its words). */
  .evidence {
    padding: 0 var(--space-8) var(--space-4) calc(var(--space-8) + var(--icon-sm) + var(--space-8));
    color: var(--text-subtle);
    font: var(--type-sm);
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

  /* The groups of "Warum": navy sub-labels with a soft count. */
  .sub {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    color: var(--text-label);
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
