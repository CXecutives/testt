<!--
  A whole number in a text field (years, euros, percent): digits only, an empty field is no
  value. Money is grouped like everywhere in the app (`1.100`, the euro sign stands in the
  label); the grouping comes back when the field is left, so typing never moves the caret.
  Money typed with cents (`950,50` or `950.50`) counts whole euros and says so under the
  field; a group of three digits after a point or comma is a thousands separator (`1.100`,
  `1,100`). Every number field has the same width.
-->
<script lang="ts">
  import TextField from '$components/TextField.svelte';
  import { formatNumber } from '$lib/i18n/format';
  import { t } from '$lib/i18n/t';

  interface Props {
    value: number | null;
    /** Euros: grouped (`1.100`), cents cut off with a note. */
    money?: boolean;
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
    id = null,
    label = null,
    placeholder = null,
    invalid = false,
    describedby = null,
    testid = null,
  }: Props = $props();

  const noteId = $props.id();

  /** A typed text as a whole number and whether cents were cut off. */
  function read(text: string): { value: number | null; cents: boolean } {
    const cents = money ? /[.,]\d{1,2}$/.exec(text) : null;
    const whole = cents ? text.slice(0, cents.index) : text;
    const digits = whole.replace(/\D/g, '');
    return {
      value: digits === '' ? null : Number(digits.slice(0, 9)),
      cents: cents !== null,
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
  <TextField
    bind:value={text}
    {id}
    {label}
    {placeholder}
    {invalid}
    describedby={rounded ? `${noteId}-rounded` : describedby}
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
{#if rounded}
  <p class="note" id="{noteId}-rounded" data-testid={testid ? `${testid}-rounded` : undefined}>
    {t.profile.field.rounded}
  </p>
{/if}

<style>
  .number {
    display: block;
    max-width: var(--stat-min);
  }

  .note {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
