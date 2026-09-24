<!--
  The languages: one row each with the language and its level (A1 to C2 or Muttersprache as
  toggle buttons; pressing the chosen level again clears it), then "Sprache hinzufügen".
  Enter moves through the rows like in the competences (rows.ts); it never saves.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import { formKeys } from '$lib/input/input';
  import type { LanguageLevel, ProfileLanguage } from '$lib/ipc/types';
  import { tick } from 'svelte';
  import ChoiceButtons from './ChoiceButtons.svelte';
  import { enterRow, focusRow } from './rows';

  interface Props {
    rows: ProfileLanguage[];
  }

  let { rows = $bindable() }: Props = $props();

  const t = de.profile.field;
  const LEVELS = Object.entries(de.profile.level).map(([level, label]) => ({ id: level, label }));
  let list = $state<HTMLElement | null>(null);

  const append = (): void => {
    rows = [...rows, { language: '', level: null, origin: null }];
  };
  const remove = (row: ProfileLanguage): void => {
    rows = rows.filter((other) => other !== row);
  };

  async function add(): Promise<void> {
    append();
    await tick();
    focusRow(list, rows.length - 1);
  }

  const enter = (row: ProfileLanguage): void =>
    void enterRow({
      list,
      rows,
      row,
      blank: (r) => r.language.trim() === '' && r.level === null,
      add: append,
      remove,
    });
</script>

<div class="list" bind:this={list} data-testid="languages">
  {#each rows as row (row)}
    <div class="row" data-row data-testid="language-row" use:formKeys={{ save: () => enter(row) }}>
      <span class="name">
        <TextField
          bind:value={row.language}
          label={t.language}
          placeholder={rows.length === 1 ? t.languagePlaceholder : null}
          testid="language-name"
        />
      </span>
      <ChoiceButtons
        options={LEVELS}
        selected={row.level === null ? [] : [row.level]}
        label={t.level}
        testid="language-level"
        onchange={(next) => (row.level = (next[0] as LanguageLevel | undefined) ?? null)}
      />
      <span class="remove">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={t.removeLanguage(row.language.trim())}
          testid="language-remove"
          onclick={() => remove(row)}
        />
      </span>
    </div>
  {/each}
  <span>
    <Button
      variant="secondary"
      size="sm"
      icon="plus"
      label={t.addLanguage}
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
</style>
