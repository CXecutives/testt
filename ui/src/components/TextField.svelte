<!--
  Single-line field: text | password (with show/hide) | search (with clear). Spellcheck,
  autocorrect and autocapitalize are off. Use inside Field for label, hint and error.
-->
<script lang="ts">
  import { FIELD_ATTRIBUTES } from '$lib/input/input';
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
</script>

<div class="field {kind}" class:invalid class:disabled>
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
        onclick={() => (revealed = !revealed)}
      />
    </span>
  {:else if kind === 'search' && value !== ''}
    <span class="trail">
      <Button variant="ghost" size="sm" iconOnly icon="x" label={de.field.clear} onclick={clear} />
    </span>
  {/if}
</div>

<style>
  .field {
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
    box-shadow: var(--glow-sm);
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

  .search .input {
    padding-left: var(--space-8);
  }

  .lead {
    display: inline-flex;
    padding-left: var(--space-12);
    color: var(--text-subtle);
  }

  .trail {
    display: inline-flex;
    padding-right: var(--space-4);
  }
</style>
