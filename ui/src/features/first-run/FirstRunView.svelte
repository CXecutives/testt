<!--
  First run (full page): the app mark on a soft coral wash, one sentence of what the app
  does, one about privacy, and three real steps that tick themselves: connect the mailbox,
  choose a profile (or save a template first), fetch. The next open step carries the one
  primary button; "Abrufen" stays locked with its reason until a mailbox is connected.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Icon from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import { app } from '$lib/state/app.svelte';
  import { run } from '$lib/state/run.svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';

  const mailboxDone = $derived(app.hasMailbox);
  const profileDone = $derived(app.state?.profile != null);
  /** The step whose action is the primary one. */
  const current = $derived(!mailboxDone ? 1 : !profileDone ? 2 : 3);
  let profileNote = $state<{ tone: 'success' | 'danger'; text: string } | null>(null);
  let busy = $state(false);

  async function pick(): Promise<void> {
    profileNote = null;
    busy = true;
    try {
      if ((await invoke('pick_profile')) !== null) await app.load();
    } catch (error) {
      profileNote = { tone: 'danger', text: errorText(error) };
    } finally {
      busy = false;
    }
  }

  async function template(): Promise<void> {
    profileNote = null;
    try {
      if ((await invoke('save_profile_template')) !== null) {
        profileNote = { tone: 'success', text: de.profile.templateSaved };
      }
    } catch (error) {
      profileNote = { tone: 'danger', text: errorText(error) };
    }
  }
</script>

{#snippet marker(step: number, done: boolean)}
  <span class="marker" class:done class:current={current === step && !done}>
    {#if done}<Icon name="check" size="sm" />{:else}{step}{/if}
  </span>
{/snippet}

<div class="hero" data-testid="first-run">
  <div class="column">
    <header class="intro">
      <BrandMark size="lg" />
      <h1 class="title">{de.app.name}</h1>
      <p class="benefit">{de.firstRun.benefit}</p>
      <p class="privacy"><Icon name="shield" size="sm" />{de.firstRun.privacy}</p>
    </header>

    <Card padding="lg">
      <ol class="steps" aria-label={de.firstRun.steps}>
        <li class="step" data-testid="step-mailbox" data-done={mailboxDone}>
          {@render marker(1, mailboxDone)}
          <div class="body">
            <h2 class="name">{de.firstRun.mailbox}</h2>
            {#if mailboxDone}
              <p class="done-text">{app.state?.mailbox.user}</p>
            {:else}
              <MailboxForm saveLabel={de.settings.connect} />
            {/if}
          </div>
        </li>

        <li class="step" data-testid="step-profile" data-done={profileDone}>
          {@render marker(2, profileDone)}
          <div class="body">
            <h2 class="name">{de.firstRun.profile}</h2>
            {#if profileDone}
              <p class="done-text">{app.state?.profile?.fileName}</p>
            {:else}
              <p class="hint">{de.firstRun.profileOr}</p>
              <div class="actions">
                <Button
                  variant={current === 2 ? 'primary' : 'secondary'}
                  icon="file-up"
                  label={de.profile.pick}
                  loading={busy}
                  testid="first-pick-profile"
                  onclick={() => void pick()}
                />
                <Button
                  variant="ghost"
                  icon="download"
                  label={de.profile.template}
                  testid="first-template"
                  onclick={() => void template()}
                />
              </div>
              {#if profileNote}
                <Notice tone={profileNote.tone} variant="inline" text={profileNote.text} />
              {/if}
            {/if}
          </div>
        </li>

        <li class="step" data-testid="step-fetch">
          {@render marker(3, false)}
          <div class="body">
            <h2 class="name">{de.firstRun.fetch}</h2>
            <p class="hint">{de.firstRun.fetchHint}</p>
            <div class="actions">
              <Button
                variant={current === 3 ? 'primary' : 'secondary'}
                icon="refresh-cw"
                label={de.toolbar.fetch}
                disabled={!mailboxDone}
                disabledReason={de.toolbar.needsMailbox}
                testid="first-fetch"
                onclick={() => void run.start({ kind: 'fetch' })}
              />
            </div>
            {#if run.startError}
              <Notice tone="danger" variant="inline" text={run.startError} />
            {/if}
          </div>
        </li>
      </ol>
    </Card>
  </div>
</div>

<style>
  .hero {
    min-height: 100%;
    padding: var(--space-48) var(--space-24);
    background: var(--grad-hero);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    max-width: var(--reader-width);
    margin: 0 auto;
  }

  .intro {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-12);
    text-align: center;
  }

  .title {
    margin-top: var(--space-8);
    color: var(--text-heading);
    font: var(--type-display);
    letter-spacing: var(--tracking-tight);
  }

  .benefit {
    max-width: var(--form-width);
    color: var(--text);
    font: var(--type-lg);
    font-weight: var(--weight-regular);
  }

  .privacy {
    display: inline-flex;
    align-items: center;
    gap: var(--space-6);
    color: var(--text-muted);
    font: var(--type-md);
  }

  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
  }

  .step {
    display: flex;
    gap: var(--space-16);
  }

  .marker {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--tile-sm);
    height: var(--tile-sm);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-full);
    color: var(--text-muted);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
    font-variant-numeric: var(--numeric);
  }

  .marker.current {
    border-color: var(--accent);
    color: var(--accent-text);
  }

  .marker.done {
    border-color: var(--success);
    background-color: var(--success-soft);
    color: var(--success-strong);
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-12);
    min-width: 0;
    padding-top: var(--space-4);
  }

  .name {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .hint,
  .done-text {
    color: var(--text-muted);
    font: var(--type-md);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }
</style>
