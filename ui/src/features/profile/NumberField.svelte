<!--
  A whole number in a text field (years, euros, percent): digits only, grouping dots are
  accepted and ignored, an empty field is no value. The unit stands in the label.
-->
<script lang="ts">
  import TextField from '$components/TextField.svelte';

  interface Props {
    value: number | null;
    id?: string | null;
    label?: string | null;
    placeholder?: string | null;
    invalid?: boolean;
    describedby?: string | null;
    testid?: string | null;
  }

  let {
    value = $bindable(),
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

  let text = $state(value === null ? '' : String(value));

  // A value set from outside (discard, a new draft) shows in the field.
  $effect(() => {
    if (parse(text) !== value) text = value === null ? '' : String(value);
  });
</script>

<TextField
  bind:value={text}
  {id}
  {label}
  {placeholder}
  {invalid}
  {describedby}
  {testid}
  oninput={(next) => {
    const clean = next.replace(/[^\d.]/g, '');
    if (clean !== next) text = clean;
    value = parse(clean);
  }}
/>
