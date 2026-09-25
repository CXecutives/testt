<!--
  A profile from a CV with an AI (a new one, or an update of the stored one): one sentence on
  where the CV goes, the prompt is on the clipboard (or can be copied again; when copying
  failed the step says so in the danger tone and the button copies) and can be read before it
  is sent, one line on what to do in the AI, then the field for its answer. "Übernehmen"
  (waiting, and saying so, until there is an answer) reads the answer (also inside a code
  block) with the same checks as a file and fills the form for review; nothing is saved yet.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Field from '$components/Field.svelte';
  import Icon from '$components/Icon.svelte';
  import TextArea from '$components/TextArea.svelte';
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import { primaryFirst } from '$lib/platform';

  interface Props {
    heading: string;
    /** The prompt as it goes to the AI (`null` while it loads). */
    prompt: string | null;
    /** The request is on the clipboard (false: copying failed). */
    copied: boolean;
    busy: boolean;
    error: string | null;
    oncopy: () => void;
    ontake: (answer: string) => void;
    oncancel: () => void;
  }

  let { heading, prompt, copied, busy, error, oncopy, ontake, oncancel }: Props = $props();

  const words = $derived(t.profile.paste);
  const id = $props.id();
  const actionFirst = primaryFirst();
  let answer = $state('');

  function take(): void {
    if (answer.trim() !== '' && !busy) ontake(answer);
  }
</script>

<Card padding="md" testid="profile-paste">
  <div class="paste" use:formKeys={{ cancel: oncancel }}>
    <div class="top">
      <h2 class="heading">{heading}</h2>
      <p class="privacy" data-testid="paste-privacy">{words.privacy}</p>
    </div>
    <ol class="steps">
      <li class="step" data-testid="paste-copied">
        <span class="mark" class:done={copied} class:failed={!copied}>
          {#if copied}<Icon name="check" size="sm" />{:else}1{/if}
        </span>
        <span class="text">{copied ? words.copied : words.copyFailed}</span>
        <Button
          variant="ghost"
          size="sm"
          icon="copy"
          label={copied ? words.copyAgain : words.copy}
          testid="paste-copy"
          onclick={oncopy}
        />
      </li>
      <li class="step">
        <span class="mark">2</span>
        <span class="text">{words.step}</span>
      </li>
    </ol>
    {#if prompt}
      <Disclosure label={words.preview} testid="paste-preview">
        <pre class="prompt" data-copy data-testid="paste-prompt">{prompt}</pre>
      </Disclosure>
    {/if}
    <Field label={words.answer} for="{id}-answer" {error}>
      <TextArea
        id="{id}-answer"
        bind:value={answer}
        rows={10}
        invalid={error !== null}
        describedby="{id}-answer-message"
        testid="paste-answer"
      />
    </Field>
    <div class="actions">
      {#snippet cancel()}
        <Button
          variant="secondary"
          label={t.common.cancel}
          testid="paste-cancel"
          onclick={oncancel}
        />
      {/snippet}
      {#if !actionFirst}{@render cancel()}{/if}
      <Button
        variant="primary"
        label={words.take}
        loading={busy}
        disabled={answer.trim() === ''}
        disabledReason={words.takeEmpty}
        testid="paste-take"
        onclick={take}
      />
      {#if actionFirst}{@render cancel()}{/if}
    </div>
  </div>
</Card>

<style>
  .paste {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }

  .top {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
  }

  .privacy {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .step {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-height: var(--control-sm);
    color: var(--text);
    font: var(--type-md);
  }

  /* The step marks of the first run: 28 px, 13 px digits, done with a green edge. */
  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-full);
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-semibold);
    font-variant-numeric: var(--numeric);
  }

  .mark.done {
    border-color: var(--success);
    background-color: var(--success-soft);
    color: var(--success-strong);
  }

  .mark.failed {
    border-color: var(--danger-strong);
    background-color: var(--danger-soft);
    color: var(--danger-strong);
  }

  /* The prompt as it goes out: its own lines, scrolled inside when long. */
  .prompt {
    max-height: calc(var(--control-md) * 8);
    overflow: auto;
    padding: var(--space-12);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-control);
    background-color: var(--surface-muted);
    color: var(--text);
    font: var(--type-sm);
    white-space: pre-wrap;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-12);
  }
</style>
