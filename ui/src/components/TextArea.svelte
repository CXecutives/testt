<!--
  Multi-line field for a longer text to paste or write (Claude's answer in the Profil view).
  Enter makes a new line; spellcheck, autocorrect and autocapitalize are off, like in every
  field. Use inside Field for label, hint and error.
-->
<script lang="ts">
  import { FIELD_ATTRIBUTES } from '$lib/input/input';
  import { describedBy } from '$lib/state/described';

  interface Props {
    value: string;
    /** Accessible name when the field is not wrapped in a Field with a label. */
    label?: string | null;
    placeholder?: string | null;
    id?: string | null;
    /** Visible lines before it scrolls. */
    rows?: number;
    invalid?: boolean;
    disabled?: boolean;
    /** Default: the message of its Field, while it shows one. */
    describedby?: string | null;
    testid?: string | null;
    oninput?: (value: string) => void;
  }

  let {
    value = $bindable(),
    label = null,
    placeholder = null,
    id = null,
    rows = 8,
    invalid = false,
    disabled = false,
    describedby = null,
    testid = null,
    oninput,
  }: Props = $props();

  const described = describedBy();
</script>

<textarea
  class="area"
  class:invalid
  {value}
  {rows}
  {disabled}
  id={id ?? undefined}
  aria-label={label ?? undefined}
  aria-invalid={invalid ? 'true' : undefined}
  aria-describedby={describedby ?? described() ?? undefined}
  placeholder={placeholder ?? undefined}
  data-testid={testid ?? undefined}
  spellcheck={FIELD_ATTRIBUTES.spellcheck}
  autocapitalize={FIELD_ATTRIBUTES.autocapitalize}
  autocomplete={FIELD_ATTRIBUTES.autocomplete}
  oninput={(event) => {
    value = event.currentTarget.value;
    oninput?.(value);
  }}></textarea>

<style>
  .area {
    display: block;
    width: 100%;
    min-height: var(--control-lg);
    padding: var(--space-8) var(--space-12);
    border: var(--border-width) solid var(--border-strong);
    border-radius: var(--radius-control);
    background-color: var(--surface);
    color: var(--text);
    font: var(--type-md);
    resize: vertical;
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-standard);
  }

  .area:hover {
    border-color: var(--border-input);
  }

  .area:focus {
    border-color: var(--border-focus);
  }

  .area::placeholder {
    color: var(--text-subtle);
  }

  .invalid,
  .invalid:hover {
    border-color: var(--danger-strong);
  }

  .area:disabled {
    opacity: var(--opacity-disabled);
  }
</style>
