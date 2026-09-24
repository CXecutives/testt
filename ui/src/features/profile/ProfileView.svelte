<!--
  Profil (centred 720): the file card (name, size, date, how well it reads; choose, save a
  template, remove; the rescore it triggers) and "Erkannt": competences, then label | value
  rows for the background (years, degrees), the domains it switched on and every hard
  criterion ("nicht gesetzt" when the profile leaves it open), warnings, and the keys it
  does not evaluate as one quiet sentence. File name and values select and copy like text. A file that no longer reads shows only the file card with
  the error. Without a profile an empty state sits at about 38 % of the height.
-->
<script lang="ts">
  import Badge, { type BadgeTone } from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Dialog from '$components/Dialog.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import IconTile from '$components/IconTile.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { de } from '$lib/i18n/de';
  import { formatBytes, formatDate, formatNumber } from '$lib/i18n/format';
  import { errorText, profileCriterion, warningText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { ProfileQuality, ProfileUnderstanding } from '$lib/ipc/types';
  import { app } from '$lib/state/app.svelte';
  import { jobs } from '$lib/state/jobs.svelte';
  import { run } from '$lib/state/run.svelte';
  import { toasts } from '$lib/state/toasts.svelte';

  const SHOWN = 18;
  const QUALITY_TONE: Record<ProfileQuality, BadgeTone> = {
    good: 'success',
    thin: 'warning',
    empty: 'danger',
  };

  /** Engine v3 fields that reach the view once the IPC type carries them. */
  type Understood = ProfileUnderstanding &
    Partial<{ packs: string[]; years: number | null; degrees: string[] }>;

  const profile = $derived(app.state?.profile ?? null);
  const understood = $derived<Understood | null>(profile?.understood ?? null);
  const background = $derived(
    [
      understood?.years ? de.profile.years(understood.years) : '',
      ...(understood?.degrees ?? []),
    ].filter((part) => part !== ''),
  );
  const packs = $derived((understood?.packs ?? []).map((pack) => de.profile.pack[pack] ?? pack));
  const criteria = $derived(
    (understood?.criteria ?? []).flatMap((notice) => {
      const row = profileCriterion(notice);
      return row === null ? [] : [row];
    }),
  );
  // Keys the app does not evaluate are a plain fact, not a warning.
  const IGNORED = 'ignoredKeys';
  const warnings = $derived(
    (understood?.warnings ?? []).flatMap((notice) => {
      const text = notice.code === IGNORED ? null : warningText(notice);
      return text === null ? [] : [text];
    }),
  );
  const ignored = $derived(
    (understood?.warnings ?? []).flatMap((notice) => {
      const text = notice.code === IGNORED ? warningText(notice) : null;
      return text === null ? [] : [text];
    }),
  );
  const rescoring = $derived((run.active && run.kind === 'rescore') || (profile?.pending ?? 0) > 0);

  let note = $state<{ tone: 'danger'; text: string } | null>(null);
  let picked = $state(false);
  let busy = $state<string | null>(null);
  let confirmRemove = $state(false);

  async function act(name: string, work: () => Promise<void>): Promise<void> {
    busy = name;
    note = null;
    try {
      await work();
    } catch (error) {
      note = { tone: 'danger', text: errorText(error) };
    } finally {
      busy = null;
    }
  }

  function pick(): void {
    void act('pick', async () => {
      if ((await invoke('pick_profile')) === null) return;
      picked = true;
      await app.load();
      void jobs.load(true);
      void jobs.loadOverview();
    });
  }

  function template(): void {
    void act('template', async () => {
      if ((await invoke('save_profile_template')) !== null) {
        toasts.show(de.profile.templateSaved);
      }
    });
  }

  function remove(): void {
    void act('remove', async () => {
      await invoke('remove_profile');
      confirmRemove = false;
      picked = false;
      await app.load();
      void jobs.load(true);
      void jobs.loadOverview();
    });
  }
</script>

<div class="page" data-testid="profile">
  {#if app.state === null}
    <!-- The shell shows nothing until the state is known. -->
  {:else if profile === null}
    <div class="empty">
      <EmptyState
        icon="file-text"
        tone="coral"
        heading={de.profile.none}
        text={de.profile.noneText}
        action={{ label: de.profile.pick, icon: 'file-up', onclick: pick }}
        secondary={{ label: de.profile.template, icon: 'download', onclick: template }}
        testid="profile-empty"
      />
      {#if note}
        <Notice tone={note.tone} variant="inline" text={note.text} testid="profile-note" />
      {/if}
    </div>
  {:else}
    <Card padding="md" testid="profile-file">
      <div class="file">
        <IconTile icon="file-text" size="md" />
        <div class="facts">
          <h2 class="name" data-testid="profile-name" data-copy>{profile.fileName}</h2>
          <p class="meta">
            {de.profile.meta(
              formatBytes(profile.bytes),
              profile.savedAt ? formatDate(profile.savedAt) : '',
            )}
          </p>
        </div>
        {#if profile.quality}
          <Badge label={de.profile.quality[profile.quality]} tone={QUALITY_TONE[profile.quality]} />
        {/if}
      </div>

      {#if profile.parseError}
        <Notice
          tone="danger"
          heading={de.profile.parseError}
          text={de.error.text(profile.parseError.kind, profile.parseError.params)}
          testid="profile-parse-error"
        />
      {:else if profile.quality && profile.quality !== 'good'}
        <Notice tone="warning" variant="inline" text={de.profile.qualityText[profile.quality]} />
      {/if}

      {#if rescoring}
        <p class="status" data-testid="profile-rescoring">
          <Spinner size="sm" label={null} />{de.profile.rescoring(profile.pending)}
        </p>
      {:else if picked}
        <Notice
          tone="success"
          variant="inline"
          text={de.profile.rescored}
          testid="profile-rescored"
        />
      {/if}

      <div class="actions">
        <Button
          variant="secondary"
          icon="file-up"
          label={de.profile.pick}
          loading={busy === 'pick'}
          testid="profile-pick"
          onclick={pick}
        />
        <Button
          variant="ghost"
          icon="download"
          label={de.profile.template}
          loading={busy === 'template'}
          testid="profile-template"
          onclick={template}
        />
        <span class="end">
          <Button
            variant="ghost"
            icon="trash-2"
            label={de.profile.remove}
            testid="profile-remove"
            onclick={() => (confirmRemove = true)}
          />
        </span>
      </div>
      {#if note}
        <Notice tone={note.tone} variant="inline" text={note.text} testid="profile-note" />
      {/if}
    </Card>

    {#if !profile.parseError}
      <Card padding="md" testid="profile-understood">
        <h2 class="heading">{de.profile.understood}</h2>
        {#if understood === null}
          <p class="quiet">{de.profile.notYet}</p>
        {:else}
          <section class="block">
            <h3 class="sub">
              {de.profile.competences}
              <span class="count">{formatNumber(understood.competenceCount)}</span>
            </h3>
            <div class="chips" data-testid="competences" data-copy>
              {#each understood.competences.slice(0, SHOWN) as competence, index (index)}
                <Badge label={competence} />
              {/each}
              {#if understood.competenceCount > Math.min(SHOWN, understood.competences.length)}
                <Badge
                  label={de.profile.more(
                    understood.competenceCount - Math.min(SHOWN, understood.competences.length),
                  )}
                />
              {/if}
            </div>
          </section>

          {#if background.length > 0 || packs.length > 0}
            <section class="block">
              <dl class="rows">
                {#if background.length > 0}
                  <div class="row">
                    <dt>{de.profile.background}</dt>
                    <dd data-testid="background" data-copy>{background.join(' · ')}</dd>
                  </div>
                {/if}
                {#if packs.length > 0}
                  <div class="row">
                    <dt>{de.profile.packs}</dt>
                    <dd data-testid="packs" data-copy>{packs.join(' · ')}</dd>
                  </div>
                {/if}
              </dl>
            </section>
          {/if}
          <section class="block">
            <h3 class="sub">{de.profile.criteria}</h3>
            <dl class="rows" data-testid="criteria-list">
              {#each criteria as item, index (index)}
                <div class="row">
                  <dt>{item.field}</dt>
                  <dd
                    class:unset={item.value === null}
                    data-copy={item.value === null ? undefined : ''}
                  >
                    {item.value ?? de.profile.unset}
                  </dd>
                </div>
              {/each}
            </dl>
          </section>
          {#if warnings.length > 0}
            <section class="block">
              <h3 class="sub">{de.profile.warnings}</h3>
              {#each warnings as warning, index (index)}
                <Notice tone="warning" variant="inline" text={warning} />
              {/each}
            </section>
          {/if}
          {#each ignored as text, index (index)}
            <p class="aside" data-testid="ignored-keys">{text}</p>
          {/each}
        {/if}
      </Card>
    {/if}
  {/if}
</div>

<Dialog
  bind:open={confirmRemove}
  variant="danger"
  heading={de.profile.removeHeading}
  text={de.profile.removeText}
  confirmLabel={de.profile.remove}
  busy={busy === 'remove'}
  testid="dialog-remove-profile"
  onconfirm={remove}
/>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    max-width: calc(var(--reader-width) + 2 * var(--pane-padding));
    min-height: 100%;
    margin: 0 auto;
    padding: var(--pane-padding) var(--pane-padding) var(--space-64);
  }

  /* The empty state sits at about 38 % of the height (spacers 38 : 62), not dead centre. */
  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    gap: var(--space-16);
  }

  .empty::before,
  .empty::after {
    content: '';
  }

  .empty::before {
    flex: 38;
  }

  .empty::after {
    flex: 62;
  }

  .file {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    margin-bottom: var(--space-16);
  }

  .facts {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .name {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-lg);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    color: var(--text-muted);
    font: var(--type-sm);
    font-variant-numeric: var(--numeric);
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
  }

  /* Label | value rows: the name muted in a fixed column, the value in ink. */
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .row {
    display: grid;
    grid-template-columns: var(--stat-min) minmax(0, 1fr);
    gap: var(--space-16);
    font: var(--type-md);
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    color: var(--text);
    overflow-wrap: break-word;
  }

  dd.unset {
    color: var(--text-subtle);
  }

  .aside {
    padding-top: var(--space-16);
    border-top: var(--border-width) solid var(--border);
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .status {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    margin-top: var(--space-12);
    color: var(--text-muted);
    font: var(--type-md);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8);
    margin-top: var(--space-16);
  }

  /* The ghost button's text ends on the card's edge, like the badge above it. */
  .end {
    margin-right: calc(-1 * var(--space-16));
    margin-left: auto;
  }

  .heading {
    margin-bottom: var(--space-12);
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--space-16) 0;
    border-top: var(--border-width) solid var(--border);
  }

  .block:last-child {
    padding-bottom: 0;
  }

  .sub {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--text-heading);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .count {
    color: var(--text-muted);
    font-weight: var(--weight-regular);
    font-variant-numeric: var(--numeric);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
  }
</style>
