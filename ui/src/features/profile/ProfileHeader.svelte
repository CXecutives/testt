<!--
  The head of the Profil view: the person (name and role, copyable; the file name as the
  tooltip) with the time of the last save, or the draft; how well the form reads (an honest
  badge, following the form while it changes: "Vollständig" only with competences, else
  "Wenig Inhalt" or "Ohne Kompetenzen"; "Etwas prüfen" while a value of the file does not
  read, its tooltip names them); one quiet line of what the app reads (terms, Schwerpunkte,
  its specialist vocabulary); keys the app does not read; the rescore a save starts; and the
  actions: another file, an update from a CV, the profile folder, remove. A new form offers
  the other two ways in (from a CV, a file). Drafts say once that they are to be reviewed.
-->
<script lang="ts">
  import Badge, { type BadgeTone } from '$components/Badge.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import IconTile from '$components/IconTile.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { t } from '$lib/i18n/t';
  import { formatDate, formatMoment, formatTime } from '$lib/i18n/format';
  import { warningText } from '$lib/i18n/texts';
  import type { Notice as NoticeData, ProfileInfo, ProfileQuality } from '$lib/ipc/types';
  import type { DraftOrigin } from '$lib/state/profile.svelte';

  interface Props {
    origin: DraftOrigin;
    profile: ProfileInfo | null;
    /** How well the form as it is reads. */
    quality: ProfileQuality | null;
    /** The form has competences ("Vollständig" needs them). */
    competences: boolean;
    /** Terms for the match (the engine's count, moved by the changes of the form). */
    terms: number | null;
    /** Schwerpunkte of the form. */
    focus: number;
    /** Domain packs the profile switched on. */
    packs: readonly string[];
    /** What is still to check, each in one sentence (the badge's tooltip). */
    checks: readonly string[];
    /** Warnings said here: what the form cannot change (keys the app does not read). */
    warnings: readonly NoticeData[];
    rescoring: boolean;
    /** Unsaved changes: another file or an update would replace them. */
    dirty: boolean;
    picking: boolean;
    note: string | null;
    onpick: () => void;
    onremove: () => void;
    onfromcv: () => void;
    onopenfolder: () => void;
  }

  let {
    origin,
    profile,
    quality,
    competences,
    terms,
    focus,
    packs,
    checks,
    warnings,
    rescoring,
    dirty,
    picking,
    note,
    onpick,
    onremove,
    onfromcv,
    onopenfolder,
  }: Props = $props();

  const stored = $derived(origin === 'stored' && profile !== null);
  /** The badge: honest about competences, "Etwas prüfen" in place of "Vollständig". */
  const badge = $derived.by((): { label: string; tone: BadgeTone; hint: string | null } | null => {
    if (quality === null) return null;
    if (!competences || quality === 'empty') {
      return {
        label: t.profile.quality.empty,
        tone: quality === 'empty' ? 'danger' : 'warning',
        hint: null,
      };
    }
    if (quality === 'thin') return { label: t.profile.quality.thin, tone: 'warning', hint: null };
    if (checks.length > 0) {
      return { label: t.profile.check, tone: 'warning', hint: checks.join(' ') };
    }
    return { label: t.profile.quality.good, tone: 'success', hint: null };
  });
  const packNames = $derived(packs.map((pack) => t.profile.pack[pack] ?? pack));
  /** One separator for the whole line: terms, Schwerpunkte, the vocabulary as one group. */
  const summary = $derived(
    terms === null
      ? null
      : [
          t.profile.understood(terms),
          focus > 0 ? t.profile.focusCount(focus) : '',
          packNames.length > 0 ? t.profile.packs(packNames) : '',
        ]
          .filter((part) => part !== '')
          .join(' · '),
  );
  const notes = $derived(
    warnings.flatMap((notice) => {
      const text = warningText(notice);
      return text === null ? [] : [text];
    }),
  );
  /** The person first: the name (the file name only as its tooltip), the role muted. */
  const person = $derived(profile?.form ?? null);
  /** When it was saved, in the format of every moment of the app (`21.09. 09:30`, the time
   *  alone today); a save of another year keeps its year. */
  function moment(iso: string): string {
    return new Date(iso).getFullYear() === new Date().getFullYear()
      ? formatMoment(iso)
      : `${formatDate(iso)} ${formatTime(iso)}`;
  }
  const savedAt = $derived(profile?.savedAt ? t.profile.savedAt(moment(profile.savedAt)) : null);
</script>

<Card padding="md" testid="profile-file">
  <div class="head">
    <div class="file">
      <IconTile tone="navy" icon="file-text" size="md" />
      <div class="facts">
        {#if stored && profile}
          <h2 class="name" data-copy use:tooltip={profile.fileName}>
            <span data-testid="profile-name">{person?.name || t.profile.unnamed}</span>
            {#if person?.title}<span class="role" data-testid="profile-role">{person.title}</span
              >{/if}
          </h2>
          {#if savedAt}<p class="meta" data-testid="profile-saved-at">{savedAt}</p>{/if}
        {:else}
          <h2 class="name" data-testid="profile-name">
            {t.profile.draft[origin === 'stored' ? 'new' : origin]}
          </h2>
        {/if}
      </div>
      {#if badge}
        <span class="quality" data-testid="profile-quality">
          <Badge label={badge.label} tone={badge.tone} hint={badge.hint} />
        </span>
      {/if}
    </div>

    {#if summary}
      <p class="summary" data-testid="profile-understood">{summary}</p>
    {/if}
    {#each notes as text, index (index)}
      <Notice tone="info" variant="inline" {text} testid="profile-warning" />
    {/each}
    {#if origin === 'file' || origin === 'answer' || origin === 'update'}
      <Notice tone="info" variant="inline" text={t.profile.review} testid="profile-review" />
    {/if}
    {#if rescoring}
      <p class="status" data-testid="profile-rescoring">
        <Spinner size="sm" label={null} />{t.profile.rescoring(profile?.pending ?? 0)}
      </p>
    {/if}

    {#if stored}
      <div class="actions">
        <Button
          variant="secondary"
          size="sm"
          icon="file-up"
          label={t.profile.pickOther}
          loading={picking}
          disabled={dirty}
          disabledReason={t.profile.leaveText}
          testid="profile-pick"
          onclick={onpick}
        />
        <Button
          variant="secondary"
          size="sm"
          icon="clipboard-paste"
          label={t.profile.updateFromCv}
          disabled={dirty}
          disabledReason={t.profile.leaveText}
          testid="profile-update-cv"
          onclick={onfromcv}
        />
        <Button
          variant="secondary"
          size="sm"
          icon="folder-open"
          label={t.common.openFolder}
          testid="profile-folder"
          onclick={onopenfolder}
        />
        <span class="end">
          <Button
            variant="ghost"
            size="sm"
            icon="trash-2"
            label={t.profile.remove}
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
          icon="clipboard-paste"
          label={t.profile.fromCv}
          disabled={dirty}
          disabledReason={t.profile.leaveText}
          testid="profile-from-cv"
          onclick={onfromcv}
        />
        <Button
          variant="secondary"
          size="sm"
          icon="file-up"
          label={t.profile.pick}
          loading={picking}
          disabled={dirty}
          disabledReason={t.profile.leaveText}
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

  /* The badge's box, no line around it (it stays centred on the person). */
  .quality {
    display: flex;
  }

  .role {
    margin-left: var(--space-8);
    color: var(--text-muted);
    font: var(--type-md);
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
    margin-right: calc(-1 * var(--ghost-inset));
    margin-left: auto;
  }
</style>
