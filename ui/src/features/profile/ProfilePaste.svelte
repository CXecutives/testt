<!--
  A profile from a CV with Claude: the request is on the clipboard (or can be copied again),
  one line on what to do in Claude, then the field for Claude's answer. "Übernehmen" reads
  the answer (also inside a code block) with the same checks as a file and fills the form
  for review; nothing is saved yet.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Card from '$components/Card.svelte';
  import Field from '$components/Field.svelte';
  import Icon from '$components/Icon.svelte';
  import TextArea from '$components/TextArea.svelte';
  import { de } from '$lib/i18n/de';
  import { formKeys } from '$lib/input/input';
  import { primaryFirst } from '$lib/platform';

  interface Props {
    /** The request is on the clipboard (false: copying failed). */
    copied: boolean;
    busy: boolean;
    error: string | null;
    oncopy: () => void;
    ontake: (answer: string) => void;
    oncancel: () => void;
  }

  let { copied, busy, error, oncopy, ontake, oncancel }: Props = $props();

  const t = de.profile.paste;
  const id = $props.id();
  const actionFirst = primaryFirst();
  let answer = $state('');

  function take(): void {
    if (answer.trim() !== '' && !busy) ontake(answer);
  }
</script>

<Card padding="lg" testid="profile-paste">
  <div class="paste" use:formKeys={{ cancel: oncancel }}>
    <h2 class="heading">{de.profile.fromCv}</h2>
    <ol class="steps">
      <li class="step" data-testid="paste-copied">
        <span class="mark" class:done={copied}>
          {#if copied}<Icon name="check" size="sm" />{:else}1{/if}
        </span>
        <span class="text">{copied ? t.copied : t.copyFailed}</span>
        <Button
          variant="ghost"
          size="sm"
          icon="copy"
          label={t.copyAgain}
          testid="paste-copy"
          onclick={oncopy}
        />
      </li>
      <li class="step">
        <span class="mark">2</span>
        <span class="text">{t.step}</span>
      </li>
    </ol>
    <Field label={t.answer} for="{id}-answer" {error}>
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
          label={de.common.cancel}
          testid="paste-cancel"
          onclick={oncancel}
        />
      {/snippet}
      {#if !actionFirst}{@render cancel()}{/if}
      <Button
        variant="primary"
        label={t.take}
        loading={busy}
        disabled={answer.trim() === ''}
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

  .heading {
    color: var(--text-heading);
    font: var(--type-lg);
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

  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-lg);
    height: var(--icon-lg);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-full);
    color: var(--text-muted);
    font: var(--type-xs);
    font-variant-numeric: var(--numeric);
  }

  .mark.done {
    border-color: var(--success-soft);
    background-color: var(--success-soft);
    color: var(--success-strong);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-12);
  }
</style>
