<!--
  The languages: one row each with the language and its level (A1 to C2 or Muttersprache as
  toggle buttons; pressing the chosen level again clears it), then "Sprache hinzufügen".
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
  import type { LanguageLevel, ProfileLanguage } from '$lib/ipc/types';
  import { tick } from 'svelte';
  import ChoiceButtons from './ChoiceButtons.svelte';

  interface Props {
    rows: ProfileLanguage[];
  }

  let { rows = $bindable() }: Props = $props();

  const words = $derived(t.profile.field);
  const LEVELS = $derived(
    Object.entries(t.profile.level).map(([level, label]) => ({ id: level, label })),
  );
  let list = $state<HTMLElement | null>(null);

  async function add(): Promise<void> {
    rows = [...rows, { language: '', level: null, origin: null }];
    await tick();
    const added = [...(list?.querySelectorAll<HTMLElement>('[data-row]') ?? [])].at(-1);
    added?.querySelector('input')?.focus();
  }
</script>

<div class="list" bind:this={list} data-testid="languages">
  {#each rows as row (row)}
    <div class="row" data-row data-testid="language-row">
      <span class="name">
        <TextField
          bind:value={row.language}
          label={words.language}
          placeholder={rows.length === 1 ? words.languagePlaceholder : null}
          testid="language-name"
        />
      </span>
      <ChoiceButtons
        options={LEVELS}
        selected={row.level === null ? [] : [row.level]}
        label={words.level}
        testid="language-level"
        onchange={(next) => (row.level = (next[0] as LanguageLevel | undefined) ?? null)}
      />
      <span class="remove">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={words.removeLanguage(row.language.trim())}
          testid="language-remove"
          onclick={() => (rows = rows.filter((other) => other !== row))}
        />
      </span>
    </div>
  {/each}
  <span>
    <Button
      variant="ghost"
      size="sm"
      label={words.addLanguage}
      testid="language-add"
      onclick={() => void add()}
    />
  </span>
</div>

<style>
  .list {
    --language-name: minmax(var(--space-64), var(--stat-min));

    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-8);
  }

  .row {
    display: grid;
    grid-template-columns: var(--language-name) minmax(0, 1fr) var(--control-sm);
    align-items: center;
    gap: var(--space-12);
    width: 100%;
  }

  .remove {
    display: flex;
    align-items: center;
  }

  .list > span:last-child {
    margin-left: calc(-1 * var(--space-12));
  }
</style>
