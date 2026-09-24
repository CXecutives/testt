<!--
  The head of the Profil view: the file (name, size and date, copyable) or the unsaved
  draft, how well it reads (badge), one quiet line of what the app understood (competences,
  Schwerpunkte, domains), what it could not use, the rescore a save starts, and the file
  actions (choose another file, remove). A new form offers the other two ways in (from a CV,
  a file). Drafts say once that they are to be reviewed. "Gut lesbar" only stands without a
  warning (otherwise "Bitte prüfen"). During the first run a saved profile leads on to the
  first fetch.
-->
<script lang="ts">
  import Badge, { type BadgeTone } from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import IconTile from '$components/IconTile.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { de } from '$lib/i18n/de';
  import { formatBytes, formatDate } from '$lib/i18n/format';
  import { warningText } from '$lib/i18n/texts';
  import type { ProfileInfo, ProfileQuality } from '$lib/ipc/types';
  import type { DraftOrigin } from '$lib/state/profile.svelte';

  interface Props {
    origin: DraftOrigin;
    profile: ProfileInfo | null;
    quality: ProfileQuality | null;
    rescoring: boolean;
    rescored: boolean;
    /** Unsaved changes: another file would replace them. */
    dirty: boolean;
    picking: boolean;
    note: string | null;
    onpick: () => void;
    onremove: () => void;
    onfromcv: () => void;
    /** The way on after the first save during the first run; `null` otherwise. */
    onnext: (() => void) | null;
  }

  let {
    origin,
    profile,
    quality,
    rescoring,
    rescored,
    dirty,
    picking,
    note,
    onpick,
    onremove,
    onfromcv,
    onnext,
  }: Props = $props();

  const QUALITY_TONE: Record<ProfileQuality, BadgeTone> = {
    good: 'success',
    thin: 'warning',
    empty: 'danger',
  };
  /** Said where it helps: the quality at the competences, empty criteria at their section. */
  const SHOWN_ELSEWHERE = new Set(['fewCompetences', 'noCompetences', 'noCriteria']);

  const stored = $derived(origin === 'stored' && profile !== null);
  const understood = $derived(stored ? (profile?.understood ?? null) : null);
  const packs = $derived((understood?.packs ?? []).map((pack) => de.profile.pack[pack] ?? pack));
  /** One separator for the whole line: count, Schwerpunkte, the domains as one group. */
  const summary = $derived(
    understood === null
      ? null
      : [
          de.profile.understood(understood.competenceCount),
          understood.focus.length > 0 ? de.profile.focusCount(understood.focus.length) : '',
          packs.length > 0 ? de.profile.packs(packs) : '',
        ]
          .filter((part) => part !== '')
          .join(' · '),
  );
  const warnings = $derived(
    (understood?.warnings ?? []).flatMap((notice) => {
      const text = SHOWN_ELSEWHERE.has(notice.code) ? null : warningText(notice);
      return text === null ? [] : [text];
    }),
  );
</script>

<Card padding="md" testid="profile-file">
  <div class="head">
    <div class="file">
      <IconTile tone="navy" icon="file-text" size="md" />
      <div class="facts">
        {#if stored && profile}
          <h2 class="name" data-testid="profile-name" data-copy>{profile.fileName}</h2>
          <p class="meta">
            {de.profile.meta(
              formatBytes(profile.bytes),
              profile.savedAt ? formatDate(profile.savedAt) : '',
            )}
          </p>
        {:else}
          <h2 class="name" data-testid="profile-name">
            {de.profile.draft[origin === 'stored' ? 'new' : origin]}
          </h2>
          <p class="meta">{de.profile.unsaved}</p>
        {/if}
      </div>
      {#if quality === 'good' && warnings.length > 0}
        <Badge label={de.profile.check} tone="warning" />
      {:else if quality}
        <Badge label={de.profile.quality[quality]} tone={QUALITY_TONE[quality]} />
      {/if}
    </div>

    {#if summary}
      <p class="summary" data-testid="profile-understood">{summary}</p>
    {/if}
    {#each warnings as warning, index (index)}
      <Notice tone="warning" variant="inline" text={warning} testid="profile-warning" />
    {/each}
    {#if origin === 'file' || origin === 'answer'}
      <Notice tone="info" variant="inline" text={de.profile.review} testid="profile-review" />
    {/if}
    {#if rescoring}
      <p class="status" data-testid="profile-rescoring">
        <Spinner size="sm" label={null} />{de.profile.rescoring(profile?.pending ?? 0)}
      </p>
    {:else if rescored}
      <Notice
        tone="success"
        variant="inline"
        text={de.profile.rescored}
        testid="profile-rescored"
      />
    {/if}
    {#if onnext}
      <span>
        <Button
          variant="secondary"
          size="sm"
          icon="refresh-cw"
          label={de.profile.next}
          testid="profile-next"
          onclick={onnext}
        />
      </span>
    {/if}

    {#if stored}
      <div class="actions">
        <Button
          variant="secondary"
          size="sm"
          icon="file-up"
          label={de.profile.pick}
          loading={picking}
          disabled={dirty}
          disabledReason={de.profile.leaveText}
          testid="profile-pick"
          onclick={onpick}
        />
        <span class="end">
          <Button
            variant="ghost"
            size="sm"
            icon="trash-2"
            label={de.profile.remove}
            testid="profile-remove"
            onclick={onremove}
          />
        </span>
      </div>
    {:else if origin === 'new'}
      <div class="actions">
        <Button
          variant="secondary"
          size="sm"
          label={de.profile.fromCv}
          disabled={dirty}
          disabledReason={de.profile.leaveText}
          testid="profile-from-cv"
          onclick={onfromcv}
        />
        <Button
          variant="ghost"
          size="sm"
          icon="file-up"
          label={de.profile.pick}
          loading={picking}
          disabled={dirty}
          disabledReason={de.profile.leaveText}
          testid="profile-pick"
          onclick={onpick}
        />
      </div>
    {/if}
    {#if note}
      <Notice tone="danger" variant="inline" text={note} testid="profile-note" />
    {/if}
  </div>
</Card>

<style>
  .head {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .file {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }

  .facts {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .name {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-lg);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta,
  .summary {
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .summary {
    padding-top: var(--space-12);
    border-top: var(--border-width) solid var(--border);
  }

  .status {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
  }

  /* The ghost button's text ends on the card's edge, like the badge above it. */
  .end {
    margin-right: calc(-1 * var(--space-12));
    margin-left: auto;
  }
</style>
