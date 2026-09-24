<!--
  A list of short values as chips in a field: type and press Enter (or leave the field) to
  add, x removes, Backspace in the empty field removes the last one, Esc drops what was
  typed. A pasted comma, semicolon or line list becomes one chip per entry. A value that
  is already there (in any case) is not added twice. Without `entry` the field only shows
  and removes (chips chosen elsewhere). Keys come from input.ts (chipKeys).
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { chipKeys, FIELD_ATTRIBUTES } from '$lib/input/input';
  import Icon from './Icon.svelte';

  interface Props {
    values: string[];
    /** Accessible name when the field is not wrapped in a Field with a label. */
    label?: string | null;
    placeholder?: string | null;
    /** id of the text input, for the label of a Field. */
    id?: string | null;
    describedby?: string | null;
    invalid?: boolean;
    /** The field takes typed values (off: it only shows and removes). */
    entry?: boolean;
    testid?: string | null;
    onchange?: (values: string[]) => void;
  }

  let {
    values = $bindable(),
    label = null,
    placeholder = null,
    id = null,
    describedby = null,
    invalid = false,
    entry = true,
    testid = null,
    onchange,
  }: Props = $props();

  const SEPARATORS = /[,;\n\r\t]+/;

  let draft = $state('');
  let input = $state<HTMLInputElement | null>(null);

  const known = (list: string[], text: string): boolean =>
    list.some((value) => value.toLowerCase() === text.toLowerCase());

  function update(next: string[]): void {
    values = next;
    onchange?.(next);
  }

  /** Adds every entry of `text` that is not there yet; `true` if there was text. */
  function add(text: string): boolean {
    const parts = text
      .split(SEPARATORS)
      .map((part) => part.trim())
      .filter((part) => part !== '');
    if (parts.length === 0) return false;
    const next = [...values];
    for (const part of parts) {
      if (!known(next, part)) next.push(part);
    }
    if (next.length !== values.length) update(next);
    return true;
  }

  function commit(): boolean {
    const added = add(draft);
    draft = '';
    return added;
  }

  function remove(index: number): void {
    update(values.filter((_, at) => at !== index));
    if (entry) input?.focus();
  }

  const keys = {
    commit,
    removeLast: (): boolean => {
      if (draft !== '' || values.length === 0) return false;
      update(values.slice(0, -1));
      return true;
    },
    clear: (): boolean => {
      if (draft === '') return false;
      draft = '';
      return true;
    },
  };

  /** A pasted list becomes chips at once; a single value is pasted as text. */
  function paste(event: ClipboardEvent): void {
    const text = event.clipboardData?.getData('text') ?? '';
    if (!SEPARATORS.test(text.trim())) return;
    event.preventDefault();
    add(`${draft},${text}`);
    draft = '';
  }

  /** A press on the free area of the field puts the caret into its input. */
  function focusInput(event: PointerEvent): void {
    if (!entry || event.button !== 0 || event.target !== event.currentTarget) return;
    event.preventDefault();
    input?.focus();
  }
</script>

<div
  class="field"
  class:invalid
  class:entry
  class:filled={values.length > 0}
  role="presentation"
  data-testid={testid ?? undefined}
  onpointerdown={focusInput}
>
  {#each values as value, index (value)}
    <span class="chip">
      <span class="text">{value}</span>
      <button
        type="button"
        class="remove"
        tabindex="-1"
        data-keep-focus
        aria-label={de.chips.remove(value)}
        onclick={() => remove(index)}
      >
        <Icon name="x" size="xs" />
      </button>
    </span>
  {/each}
  {#if entry}
    <input
      bind:this={input}
      bind:value={draft}
      class="input"
      type="text"
      id={id ?? undefined}
      aria-label={label ?? undefined}
      aria-invalid={invalid ? 'true' : undefined}
      aria-describedby={describedby ?? undefined}
      placeholder={values.length === 0 ? (placeholder ?? undefined) : undefined}
      spellcheck={FIELD_ATTRIBUTES.spellcheck}
      autocorrect={FIELD_ATTRIBUTES.autocorrect}
      autocapitalize={FIELD_ATTRIBUTES.autocapitalize}
      autocomplete={FIELD_ATTRIBUTES.autocomplete}
      use:chipKeys={keys}
      onpaste={paste}
      onblur={() => commit()}
    />
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    min-width: 0;
  }

  .entry {
    min-height: var(--control-md);
    padding: var(--space-4);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-control);
    background-color: var(--surface);
    cursor: text;
    transition: border-color var(--dur-fast) var(--ease-standard);
  }

  .entry:hover {
    border-color: var(--border-input);
  }

  .entry:focus-within {
    border-color: var(--focus);
    box-shadow: var(--focus-halo);
  }

  .invalid,
  .invalid:hover {
    border-color: var(--danger-strong);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    max-width: 100%;
    min-height: calc(var(--control-sm) - var(--space-4));
    padding: 0 var(--space-2) 0 var(--space-8);
    border-radius: var(--radius-full);
    background-color: var(--active-surface);
    color: var(--active-text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
    cursor: default;
  }

  .text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .remove {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-md);
    height: var(--icon-md);
    border-radius: var(--radius-full);
    color: var(--active-text);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .remove:hover {
    background-color: var(--count-soft-hover);
    transition-duration: var(--dur-hover);
  }

  .input {
    flex: 1 1 0;
    min-width: var(--space-48);
    height: calc(var(--control-sm) - var(--space-4));
    padding: 0 var(--space-8);
    border: 0;
    background-color: transparent;
    color: var(--text);
    font: var(--type-md);
    outline: none;
  }

  /* Next to chips the caret needs little room; while typing the field takes a line of
     its own where the chips leave too little. */
  .filled .input {
    min-width: var(--space-8);
  }

  .filled .input:focus {
    min-width: var(--space-64);
  }

  .input::placeholder {
    color: var(--text-subtle);
  }
</style>
