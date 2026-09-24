<!--
  Profil (centred 720): the file card (name, size, date, how well it reads; choose, save a
  template, remove; the rescore it triggers) and "Das hat die App verstanden": competences,
  the hard criteria in plain words, warnings and where the app read from.
-->
<script lang="ts">
  import Badge, { type BadgeTone } from '$components/Badge.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Dialog from '$components/Dialog.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Icon from '$components/Icon.svelte';
  import IconTile from '$components/IconTile.svelte';
  import Notice from '$components/Notice.svelte';
  import Spinner from '$components/Spinner.svelte';
  import { de } from '$lib/i18n/de';
  import { formatBytes, formatDate, formatNumber } from '$lib/i18n/format';
  import { errorText, profileCriterionText, warningText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import type { ProfileQuality } from '$lib/ipc/types';
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

  const profile = $derived(app.state?.profile ?? null);
  const understood = $derived(profile?.understood ?? null);
  const criteria = $derived(
    (understood?.criteria ?? []).flatMap((notice) => {
      const text = profileCriterionText(notice);
      return text === null ? [] : [{ text, set: notice.params.set !== false }];
    }),
  );
  const warnings = $derived(
    (understood?.warnings ?? []).flatMap((notice) => {
      const text = warningText(notice);
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
        heading={de.profile.none}
        text={de.profile.noneText}
        action={{ label: de.profile.pick, icon: 'file-up', onclick: pick }}
        secondary={{ label: de.profile.template, icon: 'download', onclick: template }}
        quiet={app.hasMailbox}
        testid="profile-empty"
      />
      {#if note}
        <Notice tone={note.tone} variant="inline" text={note.text} testid="profile-note" />
      {/if}
    </div>
  {:else}
    <Card padding="md" testid="profile-file">
      <div class="file">
        <IconTile icon="file-text" tone="coral" size="lg" />
        <div class="facts">
          <h2 class="name" data-testid="profile-name">{profile.fileName}</h2>
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
          <div class="chips" data-testid="competences">
            {#each understood.competences.slice(0, SHOWN) as competence, index (index)}
              <Badge label={competence} tone="coral" />
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

        <section class="block">
          <h3 class="sub">{de.profile.criteria}</h3>
          <ul class="criteria" data-testid="criteria-list">
            {#each criteria as item, index (index)}
              <li class="criterion" class:unset={!item.set}>
                <Icon name={item.set ? 'check' : 'minus'} size="sm" />
                <span>{item.text}</span>
              </li>
            {/each}
          </ul>
        </section>

        {#if warnings.length > 0}
          <section class="block">
            <h3 class="sub">{de.profile.warnings}</h3>
            {#each warnings as warning, index (index)}
              <Notice tone="warning" variant="inline" text={warning} />
            {/each}
          </section>
        {/if}
      {/if}
    </Card>
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
    gap: var(--space-24);
    max-width: var(--reader-width);
    min-height: 100%;
    margin: 0 auto;
    padding: var(--space-32) var(--space-32) var(--space-64);
  }

  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-16);
  }

  .file {
    display: flex;
    align-items: center;
    gap: var(--space-16);
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
    font: var(--type-xl);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta,
  .quiet {
    color: var(--text-muted);
    font: var(--type-md);
    font-variant-numeric: var(--numeric);
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

  .end {
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
    padding: var(--space-20) 0;
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
    font: var(--type-md);
    font-weight: var(--weight-semibold);
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

  .criteria {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .criterion {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--text);
    font: var(--type-md);
  }

  .criterion :global(svg) {
    color: var(--success-strong);
  }

  .criterion.unset {
    color: var(--text-muted);
  }

  .criterion.unset :global(svg) {
    color: var(--text-subtle);
  }
</style>
