<!--
  The reader, unboxed on the sheet (max 720 px), headed like an issue tracker: the title
  (without gender tags), one line of facts, then one quiet match line (ring 56 counting up,
  the band word and the must requirements met, or for an excluded job the reason) with a
  strip of quiet chips for the contract type and the hard criteria (state by icon only), and
  the actions in one row. "Warum" lists what is met, what is met only in part and what is
  open (must before nice, only "Kann" carries a badge), what to check and what excludes;
  hovering a reason lights its passage in the ad text below, a click scrolls to it. Title,
  facts and the ad text are selectable and copy with Ctrl/Cmd+C (`data-copy`).
  Actions by weight: at the end of the title line the star (Favorit), Archivieren and a quiet
  close back to the day overview (below 900 px the view's back button does); below the match
  line always the same three outlined buttons: "Anzeige öffnen", "Alert-Mail öffnen"
  (disabled, saying why, without a mail) and "Prompt für KI-Bewertung kopieren" (the job as
  a prompt for any AI chat; "Prompt kopieren" where the whole label does not fit, an icon
  button where that does not fit either: the row never wraps). "Details holen" has one
  place: next to the note on the missing text, above the ad. Moving the job away from one of
  its buttons hands the focus to the same button of the next job.
  After Archivieren the next job of the list opens, and the toast can take it back. The groups of "Warum" carry navy sub-labels with a soft count; a reason
  that jumps to its passage makes the passage flash once when it has arrived. Once the
  action row has scrolled away, a compact bar sticks to the top (ring, title, open, pin):
  it fades in sliding down 4 px and leaves faster, and it cannot be clicked while hidden.
-->
<script lang="ts" module>
  /** A button of the reader had the focus when its job moved away: the same button of the
   *  next job takes it, so archive, archive, archive works from the keyboard. */
  let handoff: { testid: string; from: string; until: number } | null = null;
  /** How long the next job may take to open and still take the focus. */
  const HANDOFF_MS = 3000;
</script>

<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '$components/Button.svelte';
  import Chip from '$components/Chip.svelte';
  import Count from '$components/Count.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Icon, { type IconName } from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import ReasonItem from '$components/ReasonItem.svelte';
  import ScoreRing, { ringState } from '$components/ScoreRing.svelte';
  import { inView, scrollArea } from '$lib/actions/inView';
  import { tooltip } from '$lib/actions/tooltip';
  import type { CriterionKey, CriterionState } from '$lib/i18n/de';
  import { t } from '$lib/i18n/t';
  import { displayTitle, formatDate, formatRelative, formatTime } from '$lib/i18n/format';
  import { clock } from '$lib/state/clock.svelte';
  import {
    criterionKey,
    criterionState,
    criterionValue,
    errorText,
    noteText,
    reasonEvidence,
    workWords,
    reasonHint,
    reasonText,
  } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { JobDetail, OpenTarget, Reason } from '$lib/ipc/types';
  import { duration, isReducedMotion } from '$lib/motion/motion';
  import { app } from '$lib/state/app.svelte';
  import { jobs, keyOf } from '$lib/state/jobs.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
  import AdText from './AdText.svelte';
  import { copyText } from './prompt';
  import {
    actionsOf,
    guarded,
    hasStar,
    move,
    purge,
    seen,
    toggleStar,
    type ActionId,
  } from './actions';

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
  let article = $state<HTMLElement | null>(null);
  let actionError = $state<string | null>(null);
  /** The action row has scrolled away: the compact bar is up. */
  let compact = $state(false);
  /** The passage that flashes once after a jump to it. */
  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  /** Between two facts; an expression, so its spaces stay (a copy reads "Hamburg · Remote"). */
  const SEPARATOR = ' · ';

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
  // The strip owns the hard criteria: a check or a violation it shows as a chip is not listed
  // again under "Warum" (the unclear start, the temporary agency work).
  const chipOf = (reason: Reason): CriterionKey | null =>
    criterionKey(reason.code) ?? (reason.code === 'startVague' ? 'availability' : null);
  const chipKeys = $derived(
    new Set((match?.criteria ?? []).map((reason) => criterionKey(reason.code))),
  );
  const onStrip = (reason: Reason): boolean => {
    const key = chipOf(reason);
    return key !== null && chipKeys.has(key);
  };
  const checks = $derived(
    reasons
      .filter((r) => r.kind === 'check' && !onStrip(r))
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
    hint: string | null;
    reason: Reason | null;
  }
  const chips = $derived.by((): StripChip[] => {
    const out: StripChip[] = [];
    // The contract chip steps back when a criterion chip shows the same word (its value, as
    // "Interim" for the temporary agency criterion, or its name).
    const labels = new Set(
      (match?.criteria ?? []).flatMap((reason) => {
        const key = criterionKey(reason.code);
        return key === null ? [] : [criterionValue(reason) ?? t.reader.criterion[key].label];
      }),
    );
    if (contract && !labels.has(reasonText(contract))) {
      const unclear = contract.kind === 'check';
      out.push({
        id: contract.id,
        label: reasonText(contract),
        state: unclear ? 'unknown' : 'plain',
        icon: unclear ? 'circle-help' : 'file-text',
        hint: t.reader.contractLabel,
        reason: contract.ranges.length > 0 ? contract : null,
      });
    }
    for (const reason of match?.criteria ?? []) {
      const key = criterionKey(reason.code);
      if (key === null) continue;
      const state = criterionState(reason);
      const name = t.reader.criterion[key].label;
      const value = criterionValue(reason);
      // The chip's words say the state where it shows no value ("Tagessatz nicht genannt");
      // a value without a state is one the ad leaves open (a rate by arrangement).
      const hint =
        key === 'noAnue' && state === 'unknown'
          ? t.reader.anueCheck
          : value === null
            ? state === 'unset'
              ? null
              : t.reader.criterionHint[state](name)
            : t.reader.criterionHint[state === 'unset' ? 'open' : state](name);
      out.push({
        id: reason.id,
        // The ad's own value; what it does not mention says so, neutral.
        label: value ?? (state === 'unset' ? t.facts.notMentioned(name) : name),
        state,
        icon: STATE_ICON[state],
        hint,
        reason:
          reason.ranges.length > 0
            ? reason
            : (reasons.find(
                (r) => chipOf(r) === key && passages.some((passage) => passage.reason === r.id),
              ) ?? null),
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
      return { word: app.state?.matchPending ? t.score.pending : t.score.none, tone: 'none' };
    }
    if (match.status === 'excluded') return { word: t.score.excluded, tone: 'excluded' };
    if (match.status === 'unscorable') return { word: t.score.unscorable, tone: 'none' };
    return { word: t.score.band[match.band], tone: match.band };
  });
  const exclusion = $derived(
    match?.status === 'excluded'
      ? (noteText(job.match?.note ?? match.summary) ??
          (allViolations[0] ? reasonText(allViolations[0]) : t.reader.note.hardCriterion))
      : null,
  );
  // Why a job cannot be scored (too little text, an engine failure); without a note the ad
  // simply names no clear requirements.
  const unscorable = $derived(
    match?.status === 'unscorable'
      ? (noteText(job.match?.note ?? match.summary) ?? t.reader.noReasons)
      : null,
  );
  // A violation that says exactly what the match line says is not repeated.
  const violations = $derived(
    allViolations.filter((r) => reasonText(r) !== exclusion && !onStrip(r)),
  );

  const portalState = $derived(app.state?.portals.find((p) => p.portal === job.portal) ?? null);
  const detailKind = $derived(job.detail.kind);
  const canFetch = $derived(
    (detailKind === 'pending' ||
      detailKind === 'onRequest' ||
      detailKind === 'failed' ||
      detailKind === 'teaser') &&
      portalState?.enabled === true &&
      portalState.fetchDetails &&
      (detailKind !== 'teaser' || portalState.loginEnabled),
  );
  // Company, place, what the ad says about duration and remote share (else its work mode),
  // the portals, and the date like in the row ("gestern"), the exact moment in its tooltip.
  const facts = $derived.by((): { text: string; hint: string | null }[] => {
    const work = workWords(job.match?.facts);
    const when = job.mailDate ?? job.firstSeenAt;
    return [
      job.company,
      job.location,
      ...(work.length > 0 ? work : [job.workMode ? t.job.workMode[job.workMode] : '']),
      job.alsoOn.length > 0
        ? `${t.portal[job.portal]}, ${t.job.alsoOn(job.alsoOn.map((p) => t.portal[p]).join(', '))}`
        : t.portal[job.portal],
    ]
      .filter((fact) => fact !== '')
      .map((text) => ({ text, hint: null as string | null }))
      .concat({
        text: formatRelative(when, clock.now),
        hint: t.reader.mailAt(formatDate(when), formatTime(when)),
      });
  });

  /** No score yet and the details can be fetched: the button stands right under the band. */
  /** A score from a teaser only is a first guess. */
  const preliminary = $derived(match?.status === 'scored' && detailKind === 'teaser');

  /** The action row stays one line: where the whole label does not fit, the prompt action
   *  says only "Prompt kopieren" (its tooltip says what it copies), and where that does not
   *  fit either it is an icon button (its tooltip names it). Tried again whenever the row's
   *  width changes (before the frame is painted). */
  let actions = $state<HTMLElement | null>(null);
  let promptFit = $state<'full' | 'short' | 'icon'>('full');

  function oneLine(row: HTMLElement): boolean {
    const first = row.firstElementChild;
    const last = row.lastElementChild;
    return !(first instanceof HTMLElement && last instanceof HTMLElement)
      ? true
      : first.offsetTop === last.offsetTop;
  }

  /** The longest form of the prompt action that keeps the row on one line (the newest try
   *  wins when the width changes again meanwhile). */
  let fitting = 0;
  async function fit(row: HTMLElement): Promise<void> {
    const attempt = ++fitting;
    for (const form of ['full', 'short', 'icon'] as const) {
      if (attempt !== fitting) return;
      promptFit = form;
      await tick();
      if (attempt !== fitting || oneLine(row)) return;
    }
  }

  $effect(() => {
    const row = actions;
    if (row === null) return;
    let width = -1;
    let frame = 0;
    const observer = new ResizeObserver(([entry]) => {
      const next = entry?.contentRect.width ?? 0;
      if (next === width) return;
      width = next;
      // In the next frame, before it is painted: changing the row inside the callback
      // would make the observer report again in the same frame (a loop).
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        void fit(row);
      });
    });
    observer.observe(row);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  });

  /** Why the prompt cannot work yet (no profile to assess against, no text of the ad). */
  const promptOff = $derived(
    !app.hasProfile ? t.reader.promptNoProfile : detail.text ? null : t.reader.promptNoText,
  );

  async function copyPrompt(): Promise<void> {
    actionError = null;
    let prompt: string;
    try {
      prompt = await jobs.aiPrompt(job.key);
    } catch (error) {
      actionError = errorText(error);
      return;
    }
    if (await copyText(prompt)) toasts.show(t.toast.prompt);
    else actionError = t.reader.promptNotCopied;
  }

  /** The job's actions where it is (the same as on its row), then the star. */
  const tools = $derived(actionsOf(job.place));
  let confirmPurge = $state(false);
  let purging = $state(false);
  let purgeError = $state<string | null>(null);

  function act(id: ActionId): void {
    if (guarded()) return;
    if (id === 'purge') {
      purgeError = null;
      confirmPurge = true;
      return;
    }
    actionError = null;
    const focused = document.activeElement;
    const testid =
      focused instanceof HTMLElement && article?.contains(focused)
        ? (focused.dataset.testid ?? null)
        : null;
    // The compact bar starts hidden in the next job: its twin in the head takes the focus.
    handoff =
      testid === null
        ? null
        : {
            testid: testid.replace(/^compact-/, 'reader-'),
            from: keyOf(job.key),
            until: performance.now() + HANDOFF_MS,
          };
    void move([job], id).then((error) => {
      actionError = error;
      if (error !== null) handoff = null;
    });
  }

  // The next job, opened after a move from one of this reader's buttons: the same button of
  // this reader takes the focus, unless the user put it somewhere else meanwhile.
  $effect(() => {
    if (article === null) return;
    untrack(() => {
      const want = handoff;
      if (want === null || want.from === keyOf(job.key)) return;
      handoff = null;
      if (performance.now() > want.until) return;
      const now = document.activeElement;
      const free =
        now === null ||
        now === document.body ||
        now.closest('[data-testid="reader-pane"]') !== null;
      if (!free) return;
      article
        ?.querySelector<HTMLElement>(`[data-testid="${CSS.escape(want.testid)}"]`)
        ?.focus({ preventScroll: true });
    });
  });

  async function purgeJob(): Promise<void> {
    purging = true;
    purgeError = await purge([job]);
    purging = false;
    if (purgeError === null) confirmPurge = false;
  }

  function star(): void {
    if (!guarded()) toggleStar([job]);
  }

  /** Not in the inbox: where it lies, quietly under the title (the trash says when it goes). */
  const placeLine = $derived.by((): string | null => {
    if (job.place === 'archive') return t.place.inArchive;
    if (job.place !== 'trash') return null;
    const days = app.state?.autoEmptyTrashDays ?? 0;
    return days > 0 ? t.place.inTrashFor(days) : t.place.inTrash;
  });

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

<!-- Values joined by middle dots that copy with them ("Hamburg · 6 Monate"); a line breaks
     only between two values. -->
{#snippet dotted(items: { text: string; hint: string | null }[])}
  {#each items as item, index (index)}<wbr /><span class="fact"
      ><span class="sep" aria-hidden="true">{SEPARATOR}</span><span use:tooltip={item.hint}
        >{item.text}</span
      ></span
    >{/each}
{/snippet}

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
          detail={evidence}
          active={active === reason.id}
          onhover={(on) => hover(reason, on)}
          onselect={reason.ranges.length > 0 ? () => scrollTo(reason) : null}
        />
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet placeTools(prefix: string)}
  {#each tools as tool (tool.id)}
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      icon={tool.icon}
      label={tool.label}
      testid="{prefix}{tool.id}"
      onclick={() => act(tool.id)}
    />
  {/each}
  {#if hasStar(job.place)}
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      icon="star"
      label={job.pinned ? t.reader.unpin : t.reader.pin}
      pressed={job.pinned}
      testid="{prefix}pin"
      onclick={star}
    />
  {/if}
  {#if onclose}
    <span class="close">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="x"
        label={t.reader.close}
        testid="{prefix}close"
        onclick={onclose}
      />
    </span>
  {/if}
{/snippet}

<article
  class="reader"
  data-testid="reader"
  bind:this={article}
  onpointerdown={(event) => {
    // Only a left press counts as looking at the job (a right or middle press is no reading).
    if (event.button === 0) seen(job.key);
  }}
>
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
      <span class="compact-title">{job.title ? displayTitle(job.title) : t.job.untitled}</span>
      <span class="compact-tools">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="external-link"
          label={t.reader.open}
          testid="compact-open"
          onclick={() => openTarget({ kind: 'jobUrl', key: job.key })}
        />
        {@render placeTools('compact-')}
      </span>
    </div>
  </div>

  <header class="head">
    <div class="title-line">
      <h1 class="title" data-testid="reader-title" data-copy>
        {job.title ? displayTitle(job.title) : t.job.untitled}
      </h1>
      <span class="title-tools">
        {@render placeTools('reader-')}
      </span>
    </div>
    <p class="facts" data-copy>
      <span class="facts-line">{@render dotted(facts)}</span>
    </p>
    {#if placeLine}<p class="place-line" data-testid="place-line">{placeLine}</p>{/if}
  </header>

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
          <span class="line-inner">
            <span class="band {headline.tone}" data-testid="band">{headline.word}</span>
            {#if match && match.status === 'scored'}
              <span class="must" data-testid="must">
                {job.match && job.match.mustTotal > 0
                  ? t.reader.mustMet(job.match.mustMet, job.match.mustTotal, partialMust)
                  : t.reader.noMust}
              </span>
            {/if}
          </span>
        </p>
        {#if exclusion}
          <p class="because" data-testid="exclusion">
            {exclusion}
            <span class="inline-action">
              <Button
                variant="link"
                size="sm"
                label={t.reader.override}
                testid="override"
                onclick={() => void override()}
              />
            </span>
          </p>
        {:else if job.overridden}
          <p class="because" data-testid="overridden">
            {t.reader.overridden}
            <span class="inline-action">
              <Button
                variant="link"
                size="sm"
                label={t.reader.overrideUndo}
                testid="override-undo"
                onclick={() => void override()}
              />
            </span>
          </p>
        {:else if unscorable}
          <p class="because" data-testid="unscorable">{unscorable}</p>
        {:else if preliminary}
          <p class="because" data-testid="preliminary">{t.reader.preliminary}</p>
        {/if}
        {#if clean}
          <p class="clean" aria-label={t.reader.frame} data-testid="criteria-clean">
            <span class="strip-label">{t.reader.frame}</span>
            <span class="clean-icon"><Icon name="check" size="xs" /></span>
            <span class="clean-values" data-copy
              ><span class="facts-line"
                >{@render dotted(chips.map((chip) => ({ text: chip.label, hint: null })))}</span
              ></span
            >
          </p>
        {:else if chips.length > 0}
          <ul class="chips" aria-label={t.reader.frame} data-testid="criteria">
            <li class="strip-label">{t.reader.frame}</li>
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

  <div class="actions" bind:this={actions}>
    <Button
      variant="secondary"
      icon="external-link"
      label={t.reader.open}
      testid="open-ad"
      onclick={() => openTarget({ kind: 'jobUrl', key: job.key })}
    />
    <Button
      variant="secondary"
      icon="mail"
      label={t.reader.mail}
      disabled={!detail.mail.gmailUrl}
      disabledReason={t.reader.noMail}
      testid="open-mail"
      onclick={() => openTarget({ kind: 'gmail', key: job.key })}
    />
    <!-- An icon button names itself in its own tooltip. -->
    <span class="with-hint" use:tooltip={promptFit === 'icon' ? null : t.reader.promptHint}>
      <Button
        variant="secondary"
        icon="copy"
        iconOnly={promptFit === 'icon'}
        label={promptFit === 'short' ? t.reader.promptShort : t.reader.prompt}
        disabled={promptOff !== null}
        disabledReason={promptOff}
        testid="prompt"
        onclick={() => void copyPrompt()}
      />
    </span>
  </div>
  <span class="past-actions" use:inView={(place) => (compact = place === 'above')}></span>
  {#if actionError}
    <Notice tone="danger" variant="inline" text={actionError} />
  {/if}

  {#if match && match.status !== 'unscorable' && withRing}
    <section class="why" data-testid="why">
      <h2 class="section">{t.reader.why}</h2>
      {#if met.length + partial.length + open.length === 0}
        <p class="quiet">{t.reader.noReasons}</p>
      {:else}
        <div class="columns">
          {#if met.length + partial.length > 0}
            <div class="stack">
              {#if met.length > 0}
                <div class="group">
                  {@render sub(t.reader.met, met.length)}
                  {@render reasonList(met, 'reasons-met')}
                </div>
              {/if}
              {#if partial.length > 0}
                <div class="group">
                  {@render sub(t.reader.partial, partial.length)}
                  {@render reasonList(partial, 'reasons-partial')}
                </div>
              {/if}
            </div>
          {/if}
          {#if open.length > 0}
            <div class="group">
              {@render sub(t.reader.missing, open.length)}
              {@render reasonList(open, 'reasons-open')}
            </div>
          {/if}
        </div>
      {/if}
      {#if checks.length > 0}
        <div class="group">
          {@render sub(t.reader.check, checks.length)}
          {@render reasonList(checks, 'reasons-check')}
        </div>
      {/if}
      {#if wishes.length > 0}
        <div class="group" data-testid="wishes">
          {@render sub(t.reader.wishes, wishes.length)}
          {@render reasonList(wishes, 'reasons-wish')}
        </div>
      {/if}
      {#if violations.length > 0}
        <div class="group">
          {@render sub(t.reader.violations, violations.length)}
          {@render reasonList(violations, 'reasons-violation')}
        </div>
      {/if}
    </section>
  {/if}

  <section class="ad">
    <h2 class="section">{t.reader.ad}</h2>
    {#if detailKind !== 'ok'}
      <div class="missing">
        <Notice
          tone={detailKind === 'gone' || detailKind === 'failed' ? 'warning' : 'info'}
          variant="inline"
          text={portalState &&
          (!portalState.enabled || !portalState.fetchDetails) &&
          (detailKind === 'pending' || detailKind === 'onRequest')
            ? t.reader.detailsOff
            : detailKind === 'teaser'
              ? t.reader.teaserOf(t.portal[job.portal])
              : t.reader.detail[detailKind]}
          testid="detail-note"
        />
        {#if detailKind === 'teaser' && portalState && !portalState.loginEnabled}
          <Button
            variant="secondary"
            size="sm"
            icon="log-in"
            label={t.reader.setUpSignIn}
            testid="set-up-sign-in"
            onclick={() => navigation.go('settings')}
          />
        {/if}
        {#if canFetch}
          <Button
            variant="secondary"
            size="sm"
            icon="download"
            label={t.reader.fetchDetails}
            disabled={run.active}
            disabledReason={run.busyText}
            testid="fetch-details"
            onclick={() => void run.start({ kind: 'details', keys: [job.key] })}
          />
        {/if}
      </div>
    {:else if job.closed}
      <Notice tone="info" variant="inline" text={t.reader.closed} testid="closed-note" />
    {:else if job.short && unscorable === null}
      <Notice tone="info" variant="inline" text={t.reader.short} />
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
  bind:open={confirmPurge}
  variant="danger"
  heading={t.actions.purgeHeading(1)}
  text={t.actions.purgeText}
  confirmLabel={t.actions.purge}
  busy={purging}
  error={purgeError}
  testid="dialog-purge"
  onconfirm={() => void purgeJob()}
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
    background-color: var(--surface);
    /* The hairline lies below the bar, so its content centres on a whole pixel. */
    box-shadow: 0 var(--border-width) 0 var(--border);
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
     fact carries its dot in front, the line is shifted left by one dot). The dots are text,
     so a copy keeps them; a fact never breaks inside. */
  .facts {
    overflow: hidden;
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .facts-line {
    display: block;
    margin-left: calc(-1 * var(--space-20));
  }

  .fact {
    white-space: nowrap;
  }

  .sep {
    display: inline-block;
    width: var(--space-20);
    color: var(--text-subtle);
    text-align: center;
    white-space: pre;
  }

  /* One quiet match line: the ring, the band word with the must count, the chips. */
  .match {
    display: flex;
    align-items: center;
    gap: var(--space-16);
  }

  /* A narrow reader: the ring stands at the top of the verdict, which keeps its width. */
  @container (width < 480px) {
    .match {
      align-items: flex-start;
      gap: var(--space-12);
    }
  }

  .verdict {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }

  /* The band word and the must count; like the facts, a dot that would start a wrapped line
     is clipped. */
  .line {
    overflow: hidden;
    font: var(--type-md);
  }

  .line-inner {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    margin-left: calc(-1 * var(--space-20));
  }

  .band {
    font-weight: var(--weight-medium);
  }

  .must {
    color: var(--text-muted);
  }

  .band::before,
  .must::before {
    display: inline-block;
    width: var(--space-20);
    color: var(--text-subtle);
    font-weight: var(--weight-regular);
    text-align: center;
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

  /* The label of the strip: the size of its chips. */
  .strip-label {
    display: inline-flex;
    align-items: center;
    color: var(--text-label);
    font: var(--type-xs);
    font-weight: var(--weight-medium);
  }

  /* Every criterion met: the values in one quiet line after a green check; the label and
     the check stand on its first line when it wraps. */
  .clean {
    display: flex;
    align-items: flex-start;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .clean .strip-label,
  .clean-icon {
    flex: none;
    min-height: var(--leading-sm);
  }

  .clean-icon {
    display: inline-flex;
    align-items: center;
    color: var(--success-strong);
  }

  .clean-values {
    min-width: 0;
    overflow: hidden;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  /* The link keeps its hit area but not its height: the line stays a line of text. */
  .inline-action {
    display: inline-flex;
    margin-block: calc((var(--leading-sm) - var(--control-sm)) / 2);
    margin-left: var(--space-6);
    vertical-align: baseline;
  }

  .with-hint {
    display: inline-flex;
  }

  /* Where a job lies when it is not in the inbox: quiet, under the facts. */
  .place-line {
    color: var(--text-subtle);
    font: var(--type-sm);
  }

  /* The note on the missing text, and the way to fetch it. */
  .missing {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8) var(--space-16);
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

  .columns {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--space-16);
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

  /* The hover wash of a reason hangs out on both sides alike. */
  .reasons {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-inline: calc(-1 * var(--space-8));
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
  }
</style>
