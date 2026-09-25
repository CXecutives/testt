<!--
  A list of short values as chips in a field: type and press Enter (or leave the field) to
  add, x removes, Backspace in the empty field removes the last one, Esc drops what was
  typed, a double click on a chip takes it back into the text to edit it. A chip's value is
  copyable text (a drag selects it, Ctrl/Cmd+C copies); its x names what it removes in a
  tooltip, like every icon-only button. A list of terms
  (`split` list) also splits at commas and semicolons, typed or pasted; a list of sentences
  or names that hold commas (`split` lines) only at line breaks. A value that is already
  there (in any case) is not added twice. Without `entry` the field only shows and removes
  (chips chosen elsewhere). Keys and the double click come from input.ts (chipKeys,
  chipEdit).
  With `options` the field takes only those (the countries of the profile): typing shows the
  options whose name or other names start with it (any word of them, in any case, with or
  without accents) in a list under the field, Enter or a click takes the marked one (the
  first), leaving the field takes a single match. A chip shows its option's name; a value
  that is no option (from a file) stays and shows as it is. Text that matches nothing stays
  in the field and says so (`noMatch`).
-->
<script lang="ts" module>
  /** A value a field with options can take: its id, its name, other names to find it by. */
  export interface ChipOption {
    id: string;
    label: string;
    terms?: readonly string[];
  }

  /** Lower case without accents (`Österreich` is found by `oster` and `öster`). */
  export function folded(text: string): string {
    return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().replace(/ß/g, 'ss').trim();
  }
</script>

<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { chipEdit, chipKeys, FIELD_ATTRIBUTES, type ChipKeyHandlers } from '$lib/input/input';
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
    /** Where text splits into chips: at commas, semicolons and line breaks (terms), or only
     *  at line breaks (sentences, degrees, certificate names). */
    split?: 'list' | 'lines';
    /** The only values the field takes, suggested while typing. */
    options?: readonly ChipOption[] | null;
    /** Said under the field while the typed text matches no option. */
    noMatch?: string | null;
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
    split = 'list',
    options = null,
    noMatch = null,
    testid = null,
    onchange,
  }: Props = $props();

  const SEPARATORS = { list: /[,;\n\r\t]+/, lines: /[\n\r]+/ } as const;
  const separators = $derived(SEPARATORS[split]);
  const own = $props.id();

  let draft = $state('');
  let input = $state<HTMLInputElement | null>(null);
  /** The focus is in the field (the list of options shows only then). */
  let focused = $state(false);
  /** The marked option of the list (Enter takes it). */
  let active = $state(0);

  const known = (list: string[], text: string): boolean =>
    list.some((value) => value.toLowerCase() === text.toLowerCase());

  /** The name a chip shows: its option's, else the value itself. */
  const labelOf = (value: string): string =>
    options?.find((option) => option.id === value)?.label ?? value;

  /** Options not chosen yet whose names start with the typed text (a whole name first). */
  const matches = $derived.by((): ChipOption[] => {
    const query = folded(draft);
    if (options === null || query === '') return [];
    const rank = (option: ChipOption): number => {
      const names = [option.label, option.id, ...(option.terms ?? [])].map(folded);
      if (names.some((name) => name.startsWith(query))) return 0;
      if (names.some((name) => name.split(/[\s-]+/).some((word) => word.startsWith(query)))) {
        return 1;
      }
      return 2;
    };
    return options
      .filter((option) => !values.includes(option.id))
      .map((option) => ({ option, rank: rank(option) }))
      .filter((entry) => entry.rank < 2)
      .sort((a, b) => a.rank - b.rank || a.option.label.localeCompare(b.option.label))
      .map((entry) => entry.option);
  });
  const listed = $derived(focused && matches.length > 0);
  const nothing = $derived(options !== null && folded(draft) !== '' && matches.length === 0);

  $effect(() => {
    void draft;
    active = 0;
  });

  function update(next: string[]): void {
    values = next;
    onchange?.(next);
  }

  /** Adds every entry of `text` that is not there yet; `true` if there was text. */
  function add(text: string): boolean {
    const parts = text
      .split(separators)
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

  /** An option goes in as a chip; the typed text is done. */
  function choose(option: ChipOption): void {
    if (!values.includes(option.id)) update([...values, option.id]);
    draft = '';
  }

  /** Enter: the marked option (with options), else the typed text; `true` if there was text. */
  function commit(): boolean {
    if (options !== null) {
      if (draft.trim() === '') return false;
      const option = matches[active] ?? matches[0];
      if (option) choose(option);
      return true;
    }
    const added = add(draft);
    draft = '';
    return added;
  }

  /** The field is left: typed text becomes chips; with options only a single match. */
  function leave(): void {
    focused = false;
    if (options === null) {
      commit();
      return;
    }
    const exact = matches.filter((option) =>
      [option.label, option.id, ...(option.terms ?? [])].some(
        (name) => folded(name) === folded(draft),
      ),
    );
    const only = exact.length === 1 ? exact[0] : matches.length === 1 ? matches[0] : undefined;
    if (only) choose(only);
  }

  function remove(index: number): void {
    update(values.filter((_, at) => at !== index));
    if (entry) input?.focus();
  }

  /** The keys of input.ts; the arrows move the mark in the list of options. */
  const keys: ChipKeyHandlers & { step: (by: -1 | 1) => boolean } = {
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
    step: (by: -1 | 1): boolean => {
      if (!listed) return false;
      active = (active + by + matches.length) % matches.length;
      return true;
    },
  };

  /** A pasted list becomes chips at once; a single value is pasted as text. With options
   *  a paste is text to search. */
  function paste(event: ClipboardEvent): void {
    if (options !== null) return;
    const text = event.clipboardData?.getData('text') ?? '';
    if (!separators.test(text.trim())) return;
    event.preventDefault();
    add(`${draft}\n${text}`);
    draft = '';
  }

  /** A double click on a chip: what was typed becomes a chip, the chip's text goes back
   *  into the field with the caret at its end. */
  function edit(index: number): void {
    const value = values[index];
    if (!entry || options !== null || value === undefined) return;
    commit();
    update(values.filter((other) => other !== value));
    draft = value;
    input?.focus();
    queueMicrotask(() => input?.setSelectionRange(value.length, value.length));
  }

  /** A press on the free area of the field puts the caret into its input. */
  function focusInput(event: PointerEvent): void {
    if (!entry || event.button !== 0 || event.target !== event.currentTarget) return;
    event.preventDefault();
    input?.focus();
  }
</script>

<div class="chip-input" class:suggests={options !== null}>
  <div
    class="field"
    class:invalid
    class:entry
    class:filled={values.length > 0}
    role="presentation"
    data-testid={testid ?? undefined}
    onpointerdown={focusInput}
    use:chipEdit={entry && options === null ? edit : null}
  >
    {#each values as value, index (value)}
      <span class="chip" data-chip={index} data-value={value}>
        <span class="text" data-copy>{labelOf(value)}</span>
        <button
          type="button"
          class="remove"
          tabindex="-1"
          data-keep-focus
          aria-label={t.chips.remove(labelOf(value))}
          use:tooltip={t.chips.remove(labelOf(value))}
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
        role={options !== null ? 'combobox' : undefined}
        aria-autocomplete={options !== null ? 'list' : undefined}
        aria-expanded={options !== null ? listed : undefined}
        aria-controls={options !== null ? `${own}-options` : undefined}
        aria-activedescendant={listed ? `${own}-option-${active}` : undefined}
        aria-label={label ?? undefined}
        aria-invalid={invalid ? 'true' : undefined}
        aria-describedby={[describedby, nothing ? `${own}-none` : null]
          .filter((part) => part !== null)
          .join(' ') || undefined}
        placeholder={values.length === 0 ? (placeholder ?? undefined) : undefined}
        spellcheck={FIELD_ATTRIBUTES.spellcheck}
        autocorrect={FIELD_ATTRIBUTES.autocorrect}
        autocapitalize={FIELD_ATTRIBUTES.autocapitalize}
        autocomplete={FIELD_ATTRIBUTES.autocomplete}
        use:chipKeys={keys}
        onpaste={paste}
        onfocus={() => (focused = true)}
        onblur={leave}
      />
    {/if}
  </div>
  {#if options !== null}
    <div
      class="options"
      id="{own}-options"
      role="listbox"
      aria-label={label ?? placeholder ?? undefined}
      hidden={!listed}
      data-testid={testid ? `${testid}-options` : undefined}
    >
      {#each listed ? matches : [] as option, index (option.id)}
        <button
          type="button"
          class="option"
          class:active={index === active}
          id="{own}-option-{index}"
          role="option"
          aria-selected={index === active}
          tabindex="-1"
          data-keep-focus
          onclick={() => choose(option)}
        >
          {option.label}
        </button>
      {/each}
    </div>
    {#if nothing && noMatch}
      <p class="none" id="{own}-none" data-testid={testid ? `${testid}-none` : undefined}>
        {noMatch}
      </p>
    {/if}
  {/if}
</div>

<style>
  .chip-input {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    width: 100%;
    min-width: 0;
  }

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
    background-color: var(--active-hover);
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

  /* A field that searches its options keeps room to type next to its chips. */
  .suggests .filled .input {
    min-width: var(--space-64);
  }

  .input::placeholder {
    color: var(--text-subtle);
  }

  /* The options drop down under the field, over what follows, like a native menu. */
  .options {
    position: absolute;
    z-index: var(--z-overlay);
    top: calc(100% + var(--menu-gap));
    right: 0;
    left: 0;
    display: flex;
    flex-direction: column;
    max-height: calc(6 * var(--control-sm) + 2 * var(--space-4));
    padding: var(--space-4);
    overflow-y: auto;
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-control);
    background-color: var(--surface);
    box-shadow: var(--sh-pop);
  }

  .options[hidden] {
    display: none;
  }

  .option {
    display: flex;
    flex: none;
    align-items: center;
    height: var(--control-sm);
    padding: 0 var(--space-8);
    border-radius: var(--radius-xs);
    color: var(--text);
    font: var(--type-md);
    text-align: left;
    white-space: nowrap;
  }

  .option:hover,
  .option.active {
    background-color: var(--surface-hover);
  }

  .option.active {
    background-color: var(--active-surface);
    color: var(--active-text);
  }

  .none {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
