<!--
  A whole number in a text field (years, euros, percent): digits only, an empty field is no
  value. Money is grouped like everywhere in the app (`1.100`, the euro sign stands in the
  label); the grouping comes back when the field is left, so typing never moves the caret.
  Money typed with cents (`950,50` or `950.50`) counts whole euros and says so under the
  field; a group of three digits after a point or comma is a thousands separator (`1.100`,
  `1,100`). Every number field has the same width (or its column's, if that is narrower).
-->
<script lang="ts">
  import TextField from '$components/TextField.svelte';
  import { formatNumber } from '$lib/i18n/format';
  import { t } from '$lib/i18n/t';

  interface Props {
    value: number | null;
    /** Euros: grouped (`1.100`), cents cut off with a note. */
    money?: boolean;
    /** The unit right of the field (`€`, `Jahre`, `%`). */
    unit?: string | null;
    id?: string | null;
    label?: string | null;
    placeholder?: string | null;
    invalid?: boolean;
    describedby?: string | null;
    testid?: string | null;
  }

  let {
    value = $bindable(),
    money = false,
    unit = null,
    id = null,
    label = null,
    placeholder = null,
    invalid = false,
    describedby = null,
    testid = null,
  }: Props = $props();

  const noteId = $props.id();

  /** A typed text as a whole number and whether cents were cut off (`,00` cuts nothing). */
  function read(text: string): { value: number | null; cents: boolean } {
    const decimal = money ? /[.,](\d{1,2})$/.exec(text) : null;
    const whole = decimal ? text.slice(0, decimal.index) : text;
    const digits = whole.replace(/\D/g, '');
    return {
      value: digits === '' ? null : Number(digits.slice(0, 9)),
      cents: /[1-9]/.test(decimal?.[1] ?? ''),
    };
  }

  /** The text of a value: money grouped. */
  const shown = (number: number | null): string =>
    number === null ? '' : money ? formatNumber(number) : String(number);

  let text = $state(shown(value));
  /** The last input had cents: the note stays until the next input without them. */
  let rounded = $state(false);

  // A value set from outside (discard, a new draft) shows in the field.
  $effect(() => {
    if (read(text).value !== value) {
      text = shown(value);
      rounded = false;
    }
  });
</script>

<span class="number" role="presentation" onfocusout={() => (text = shown(value))}>
  <span class="box">
    <TextField
      bind:value={text}
      {id}
      {label}
      {placeholder}
      {invalid}
      describedby={[rounded ? `${noteId}-rounded` : describedby, unit ? `${noteId}-unit` : null]
        .filter((part) => part !== null)
        .join(' ') || null}
      {testid}
      oninput={(next) => {
        const clean = next.replace(money ? /[^\d.,]/g : /\D/g, '');
        if (clean !== next) text = clean;
        const typed = read(clean);
        rounded = typed.cents;
        value = typed.value;
      }}
    />
  </span>
  {#if unit}<span class="unit" id="{noteId}-unit">{unit}</span>{/if}
</span>
{#if rounded}
  <p class="note" id="{noteId}-rounded" data-testid={testid ? `${testid}-rounded` : undefined}>
    {t.profile.field.rounded}
  </p>
{/if}

<style>
  .number {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  /* One width for every number field, the width of the day in Verfügbarkeit too. */
  .box {
    flex: none;
    width: min(100%, calc(var(--stat-min) - var(--space-48)));
  }

  .unit {
    flex: none;
    color: var(--text-muted);
    font: var(--type-md);
  }

  .note {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
