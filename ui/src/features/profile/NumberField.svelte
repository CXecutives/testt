<!--
  A whole number in a text field (years, euros, percent): digits only, an empty field is no
  value. Money is grouped like everywhere in the app (`1.100`, the euro sign stands in the
  label); the grouping comes back when the field is left, so typing never moves the caret.
  Every number field has the same width.
-->
<script lang="ts">
  import TextField from '$components/TextField.svelte';
  import { formatNumber } from '$lib/i18n/format';

  interface Props {
    value: number | null;
    /** Euros: grouped (`1.100`). */
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

  const parse = (text: string): number | null => {
    const digits = text.replace(/\D/g, '');
    return digits === '' ? null : Number(digits.slice(0, 9));
  };

  /** The text of a value: money grouped. */
  const shown = (number: number | null): string =>
    number === null ? '' : money ? formatNumber(number) : String(number);

  let text = $state(shown(value));

  // A value set from outside (discard, a new draft) shows in the field.
  $effect(() => {
    if (parse(text) !== value) text = shown(value);
  });
</script>

<span class="number" role="presentation" onfocusout={() => (text = shown(value))}>
  <TextField
    bind:value={text}
    {id}
    {label}
    {placeholder}
    {invalid}
    {describedby}
    {testid}
    oninput={(next) => {
      const clean = next.replace(money ? /[^\d.,]/g : /\D/g, '');
      if (clean !== next) text = clean;
      value = parse(clean);
    }}
  />
</span>

<style>
  .number {
    display: block;
    max-width: var(--stat-min);
  }
</style>
