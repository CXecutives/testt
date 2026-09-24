<!--
  First run (full page) on the white sheet: the app mark, one sentence of what the app
  does, one about privacy, and three real steps that tick themselves: connect the mailbox,
  create the profile (in the Profil view), fetch. The next open step carries the one
  primary button; "Abrufen" stays locked with its reason until a mailbox is connected.
  Compact enough that the third step is in view at 1280 x 720; the sidebar is inert here.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Icon from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import { de } from '$lib/i18n/de';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';

  const mailboxDone = $derived(app.hasMailbox);
  /** Only a profile the app can score with ticks the step. */
  const profileDone = $derived(app.hasProfile);
  /** The step whose action is the primary one. */
  const current = $derived(!mailboxDone ? 1 : !profileDone ? 2 : 3);
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

    <Card padding="md">
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
              <p class="hint">{de.firstRun.profileText}</p>
              <div class="actions">
                <Button
                  variant={current === 2 ? 'primary' : 'secondary'}
                  icon="file-text"
                  label={de.profile.create}
                  testid="first-profile"
                  onclick={() => navigation.go('profile')}
                />
              </div>
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
                loading={run.starting}
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
    padding: var(--space-16) var(--space-24) var(--space-24);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    max-width: var(--reader-width);
    margin: 0 auto;
  }

  .intro {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-8);
    text-align: center;
  }

  .title {
    color: var(--text-heading);
    font: var(--type-2xl);
    letter-spacing: var(--tracking-tight);
  }

  .benefit {
    max-width: var(--measure-intro);
    color: var(--text);
    font: var(--type-lg);
    font-weight: var(--weight-regular);
    text-wrap: balance;
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
    gap: var(--space-20);
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
    font-weight: var(--weight-medium);
  }

  .marker.current {
    border-color: var(--text);
    color: var(--text);
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
    gap: var(--space-8);
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
