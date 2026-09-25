<!--
  The languages: one row each with the language and its level (A1 to C2 or Muttersprache as
  toggle buttons, each explaining itself in a tooltip; pressing the chosen level again
  clears it, and without one the app assumes B2), then "Sprache hinzufügen". Enter moves
  through the rows like in the competences (rows.ts); it never saves. A value the backend
  refused marks its row.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import TextField from '$components/TextField.svelte';
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import type { LanguageLevel, ProfileLanguage } from '$lib/ipc/types';
  import { tick } from 'svelte';
  import ChoiceButtons from './ChoiceButtons.svelte';
  import { enterRow, focusRow } from './rows';

  interface Props {
    rows: ProfileLanguage[];
    /** A row the backend refused (its place among the rows with a language) and why. */
    error?: { row: number | null; text: string } | null;
  }

  let { rows = $bindable(), error = null }: Props = $props();

  const words = $derived(t.profile.field);
  const id = $props.id();
  const LEVELS = $derived(
    (Object.keys(t.profile.level) as LanguageLevel[]).map((level) => ({
      id: level,
      label: t.profile.level[level],
      hint: t.profile.levelMeaning[level],
    })),
  );
  let list = $state<HTMLElement | null>(null);

  const refused = $derived.by((): ProfileLanguage | null => {
    if (error === null || error.row === null) return null;
    return rows.filter((row) => row.language.trim() !== '')[error.row] ?? null;
  });

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

<div class="list" bind:this={list} data-testid="languages" data-field="languages">
  {#each rows as row (row)}
    <div class="row" data-row data-testid="language-row" use:formKeys={{ save: () => enter(row) }}>
      <span class="name">
        <TextField
          bind:value={row.language}
          label={words.language}
          placeholder={rows.length === 1 ? words.languagePlaceholder : null}
          invalid={row === refused}
          describedby={row === refused ? `${id}-error` : null}
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
          onclick={() => remove(row)}
        />
      </span>
    </div>
  {/each}
  {#if error}
    <p class="error" id="{id}-error" role="alert" data-testid="language-error">{error.text}</p>
  {/if}
  <div class="foot">
    <Button
      variant="secondary"
      size="sm"
      icon="plus"
      label={words.addLanguage}
      testid="language-add"
      onclick={() => void add()}
    />
    <p class="hint">{words.levelHint}</p>
  </div>
</div>

<style>
  .list {
    --language-name: minmax(var(--space-64), var(--stat-min));

    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-8);
    container-type: inline-size;
  }

  .row {
    display: grid;
    grid-template-columns: var(--language-name) minmax(0, 1fr) var(--control-sm);
    align-items: center;
    gap: var(--space-6) var(--space-12);
    width: 100%;
  }

  .remove {
    display: flex;
    align-items: center;
    height: var(--control-md);
  }

  .error {
    color: var(--danger-strong);
    font: var(--type-sm);
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8) var(--space-16);
  }

  .hint {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  /* Narrow: the levels go in one wrapping line under the language, the x stays beside it. */
  @container (width < 520px) {
    .row {
      grid-template-columns: minmax(0, 1fr) var(--control-sm);
      align-items: start;
    }

    .row > :global([role='group']) {
      grid-column: 1 / 2;
      grid-row: 2;
    }
  }
</style>
