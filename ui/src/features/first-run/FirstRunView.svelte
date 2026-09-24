<!--
  First run (full page) on the white sheet: the app mark, one sentence of what the app
  does, one about privacy, and three real steps that tick themselves: connect the mailbox,
  a usable profile (created in the Profil view with its editor, or an existing file chosen
  here), fetch. The next open step carries the one primary button; "Abrufen" stays locked
  with its reason until a mailbox is connected. After "Alles zurücksetzen" the app starts
  here again, so this is where the reset reports. Compact enough that the third step is in
  view at 1280 x 720; the sidebar is inert here (the Profil view frees it again).

  A vertical stepper: 28 px markers (the current one deep navy, "you are here"; upcoming
  ones outlined; done ones green with a check) joined by a hairline that fills green below
  a done step. Ticking a step is a class change, so it moves only while the page is open:
  the marker cross-fades to its check, which draws itself, the line fills downwards, the
  next marker turns navy and the done text rises in. Nothing plays when the page appears.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import BrandMark from '$components/BrandMark.svelte';
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Icon from '$components/Icon.svelte';
  import Notice from '$components/Notice.svelte';
  import { de } from '$lib/i18n/de';
  import { errorText } from '$lib/i18n/texts';
  import { invoke } from '$lib/ipc/api';
  import { rise } from '$lib/motion/transitions';
  import { app } from '$lib/state/app.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { run } from '$lib/state/run.svelte';
  import MailboxForm from '../shared/MailboxForm.svelte';

  const mailboxDone = $derived(app.hasMailbox);
  /** Only a profile the engine can use counts; a broken or empty one keeps step 2 open. */
  const profileDone = $derived(app.hasProfile);
  const profile = $derived(app.state?.profile ?? null);
  /** Why an existing profile does not count yet (null: there is none, or it is fine). */
  const profileProblem = $derived(
    profile === null || profileDone
      ? null
      : profile.parseError
        ? de.profile.parseError
        : de.profile.qualityText.empty,
  );
  /** The step whose action is the primary one. */
  const current = $derived(!mailboxDone ? 1 : !profileDone ? 2 : 3);
  const reset = $derived(app.state?.resetReport ?? null);
  let profileNote = $state<{ tone: 'danger'; text: string } | null>(null);
  let busy = $state(false);

  /** Steps ticked while this page is open: only their check draws (never at mount). */
  let ticked = $state({ mailbox: false, profile: false });
  let before = untrack(() => ({ mailbox: mailboxDone, profile: profileDone }));
  $effect(() => {
    const now = { mailbox: mailboxDone, profile: profileDone };
    if (now.mailbox && !before.mailbox) ticked.mailbox = true;
    if (now.profile && !before.profile) ticked.profile = true;
    before = now;
  });

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
</script>

{#snippet marker(step: number, done: boolean, drawn: boolean)}
  <span
    class="marker"
    class:done
    class:drawn
    class:current={current === step && !done}
    aria-hidden="true"
  >
    <span class="number">{step}</span>
    <span class="check"><Icon name="check" size="sm" /></span>
  </span>
{/snippet}

{#snippet rail(step: number, done: boolean, drawn = false, last = false)}
  <div class="rail">
    {@render marker(step, done, drawn)}
    {#if !last}<span class="line"><span class="fill"></span></span>{/if}
  </div>
{/snippet}

<div class="hero" data-testid="first-run">
  <div class="column">
    <header class="intro">
      <BrandMark size="lg" />
      <h1 class="title">{de.app.name}</h1>
      <p class="benefit">{de.firstRun.benefit}</p>
      <p class="privacy"><Icon name="shield" size="sm" />{de.firstRun.privacy}</p>
    </header>

    {#if reset}
      <Notice
        tone={reset.failed > 0 ? 'warning' : 'success'}
        text={reset.failed > 0 ? de.settings.resetPartly(reset.failed) : de.settings.resetDone}
        testid="first-reset-report"
      />
    {/if}

    <Card padding="md">
      <ol class="steps" aria-label={de.firstRun.steps}>
        <li
          class="step"
          class:done={mailboxDone}
          aria-current={current === 1 ? 'step' : undefined}
          data-testid="step-mailbox"
          data-done={mailboxDone}
        >
          {@render rail(1, mailboxDone, ticked.mailbox)}
          <div class="body">
            <h2 class="name">{de.firstRun.mailbox}</h2>
            {#if mailboxDone}
              <p class="done-text" in:rise>{app.state?.mailbox.user}</p>
            {:else}
              <MailboxForm saveLabel={de.settings.connect} />
            {/if}
          </div>
        </li>

        <li
          class="step"
          class:done={profileDone}
          aria-current={current === 2 ? 'step' : undefined}
          data-testid="step-profile"
          data-done={profileDone}
        >
          {@render rail(2, profileDone, ticked.profile)}
          <div class="body">
            <h2 class="name">{de.firstRun.profile}</h2>
            {#if profileDone}
              <p class="done-text" in:rise>{profile?.fileName}</p>
            {:else}
              {#if profileProblem}
                <Notice
                  tone="warning"
                  variant="inline"
                  text={profileProblem}
                  testid="first-profile-problem"
                />
              {:else}
                <p class="hint">{de.profile.noneText}</p>
              {/if}
              <div class="actions">
                <Button
                  variant={current === 2 ? 'primary' : 'secondary'}
                  icon="user-round"
                  label={profile ? de.firstRun.openProfile : de.firstRun.createProfile}
                  testid="first-open-profile"
                  onclick={() => navigation.go('profile')}
                />
                <Button
                  variant="ghost"
                  icon="file-up"
                  label={de.profile.pick}
                  loading={busy}
                  testid="first-pick-profile"
                  onclick={() => void pick()}
                />
              </div>
              {#if profileNote}
                <Notice tone={profileNote.tone} variant="inline" text={profileNote.text} />
              {/if}
            {/if}
          </div>
        </li>

        <li class="step" aria-current={current === 3 ? 'step' : undefined} data-testid="step-fetch">
          {@render rail(3, false, false, true)}
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

  /* No gap between the steps: the hairline runs on from one marker to the next. */
  .steps {
    display: flex;
    flex-direction: column;
  }

  .step {
    display: flex;
    gap: var(--space-16);
  }

  .rail {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: center;
  }

  .line {
    position: relative;
    flex: 1;
    width: var(--border-width);
    margin: var(--space-4) 0;
    overflow: hidden;
    background-color: var(--border);
  }

  /* The done part of the line fills downwards (a transform, so never at mount). */
  .fill {
    position: absolute;
    inset: 0;
    background-color: var(--success-strong);
    transform: scaleY(0);
    transform-origin: top;
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .step.done .fill {
    transform: none;
  }

  .marker {
    display: inline-grid;
    place-items: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-full);
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-semibold);
    font-variant-numeric: var(--numeric);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  /* Number and check share the cell and cross-fade. */
  .number,
  .check {
    display: inline-flex;
    grid-area: 1 / 1;
    transition: opacity var(--dur-slow) var(--ease-standard);
  }

  .check,
  .done .number {
    opacity: 0;
  }

  .done .check {
    opacity: 1;
  }

  /* A step ticked while the page is open draws its check once (a class set by a change). */
  .drawn .check :global(path) {
    stroke-dasharray: var(--draw-length);
    animation: draw var(--dur-slow) var(--ease-out) both;
  }

  .marker.current {
    border-color: var(--surface-inverse);
    background-color: var(--surface-inverse);
    color: var(--text-inverse);
  }

  .marker.done {
    border-color: var(--success);
    background-color: var(--success-soft);
    color: var(--success-strong);
  }

  /* The heading's line centred on the 28 px marker; the space below a step keeps the line. */
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-8);
    min-width: 0;
    padding-top: var(--space-2);
    padding-bottom: var(--space-20);
  }

  .step:last-child .body {
    padding-bottom: 0;
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
