<!--
  Single-line field: text | password (with show/hide) | search (with clear). Spellcheck,
  autocorrect and autocapitalize are off. Use inside Field for label, hint and error.
  Like the native ones: the show and clear buttons are not in the Tab order and leave the
  caret in the field; a search clears on Esc, and a click on its magnifier lands in it.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { FIELD_ATTRIBUTES, formKeys } from '$lib/input/input';
  import { de } from '$lib/i18n/de';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';

  interface Props {
    value: string;
    kind?: 'text' | 'password' | 'search';
    /** Accessible name when the field is not wrapped in a Field with a label. */
    label?: string | null;
    placeholder?: string | null;
    id?: string | null;
    invalid?: boolean;
    disabled?: boolean;
    describedby?: string | null;
    testid?: string | null;
    oninput?: (value: string) => void;
  }

  let {
    value = $bindable(),
    kind = 'text',
    label = null,
    placeholder = null,
    id = null,
    invalid = false,
    disabled = false,
    describedby = null,
    testid = null,
    oninput,
  }: Props = $props();

  let revealed = $state(false);
  let input = $state<HTMLInputElement | null>(null);

  const type = $derived(kind === 'password' && !revealed ? 'password' : 'text');

  function update(next: string): void {
    value = next;
    oninput?.(next);
  }

  function clear(): void {
    update('');
    input?.focus();
  }

  /** Show or hide the password; the caret and selection stay where they were. */
  async function reveal(): Promise<void> {
    const start = input?.selectionStart ?? null;
    const end = input?.selectionEnd ?? null;
    revealed = !revealed;
    await tick();
    if (input === null) return;
    input.focus();
    // Chromium rebuilds the editor of an input whose type changed at the next style
    // update and puts the caret at the start; update now, then restore.
    void input.offsetWidth;
    if (start !== null) input.setSelectionRange(start, end ?? start);
  }

  /** Esc clears a search that has text; otherwise it goes on to the form around. */
  const keys = $derived(kind === 'search' && value !== '' ? { cancel: clear } : {});
</script>

<div class="field {kind}" class:invalid class:disabled use:formKeys={keys}>
  {#if kind === 'search'}
    <span class="lead"><Icon name="search" size="sm" /></span>
  {/if}
  <input
    bind:this={input}
    class="input"
    {type}
    {value}
    id={id ?? undefined}
    aria-label={label ?? undefined}
    aria-invalid={invalid ? 'true' : undefined}
    aria-describedby={describedby ?? undefined}
    placeholder={placeholder ?? undefined}
    {disabled}
    data-testid={testid ?? undefined}
    spellcheck={FIELD_ATTRIBUTES.spellcheck}
    autocorrect={FIELD_ATTRIBUTES.autocorrect}
    autocapitalize={FIELD_ATTRIBUTES.autocapitalize}
    autocomplete={FIELD_ATTRIBUTES.autocomplete}
    oninput={(event) => update(event.currentTarget.value)}
  />
  {#if kind === 'password'}
    <span class="trail">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon={revealed ? 'eye-off' : 'eye'}
        label={revealed ? de.field.conceal : de.field.reveal}
        inField
        onclick={() => void reveal()}
      />
    </span>
  {:else if kind === 'search' && value !== ''}
    <span class="trail">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon="x"
        label={de.field.clear}
        inField
        onclick={clear}
      />
    </span>
  {/if}
</div>

<style>
  .field {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    height: var(--control-md);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-control);
    background-color: var(--surface);
    transition: border-color var(--dur-fast) var(--ease-standard);
  }

  .field:hover {
    border-color: var(--border-input);
  }

  .field:focus-within {
    border-color: var(--focus);
    box-shadow: var(--focus-halo);
  }

  .invalid,
  .invalid:hover {
    border-color: var(--danger-strong);
  }

  .disabled {
    opacity: var(--opacity-disabled);
  }

  .input {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 var(--space-12);
    border: 0;
    background-color: transparent;
    color: var(--text);
    font: var(--type-md);
    outline: none;
  }

  .input::placeholder {
    color: var(--text-subtle);
  }

  /* The magnifier lies over the input, so a click on it lands in the field. */
  .search .input {
    padding-left: calc(var(--space-12) + var(--icon-sm) + var(--space-8));
  }

  .lead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: var(--space-12);
    display: inline-flex;
    align-items: center;
    color: var(--text-subtle);
    pointer-events: none;
  }

  .trail {
    display: inline-flex;
    padding-right: var(--space-4);
  }
</style>
