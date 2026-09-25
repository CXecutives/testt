<!--
  A value of the profile file the app could not read, said where its field is: what the file
  had, in the danger tone of a field's error, and "Wert entfernen" as the way on at the end of
  the line (the link of a field's help line). Removing it takes effect when the profile is
  saved (or, for one entry of a list, removes the entry at once).
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Icon from '$components/Icon.svelte';
  import { t } from '$lib/i18n/t';

  interface Props {
    text: string;
    /** For the `aria-describedby` of the field it belongs to. */
    id?: string | null;
    testid?: string | null;
    onremove: () => void;
  }

  let { text, id = null, testid = null, onremove }: Props = $props();
</script>

<div class="note" data-testid={testid ?? undefined}>
  <p class="text" id={id ?? undefined} role="alert">
    <Icon name="triangle-alert" size="sm" />
    <span>{text}</span>
  </p>
  <span class="action">
    <Button
      variant="link"
      size="sm"
      label={t.profile.field.removeValue}
      testid="value-remove"
      onclick={onremove}
    />
  </span>
</div>

<style>
  .note {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4) var(--space-12);
    min-height: var(--control-sm);
  }

  /* The icon sits on the first line when the text wraps (as a field's error). */
  .text {
    display: flex;
    align-items: flex-start;
    gap: var(--space-6);
    color: var(--danger-strong);
    font: var(--type-sm);
  }

  .text > :global(:first-child) {
    flex: none;
    margin-top: calc((var(--leading-sm) - var(--icon-sm)) / 2);
  }

  .action {
    display: inline-flex;
  }
</style>
