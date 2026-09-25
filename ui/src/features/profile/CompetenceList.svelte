<!--
  The core competences: one row each with the star (Schwerpunkt), the competence, its years
  and other terms for it (`auch`), then "Kompetenz hinzufügen" and the Schwerpunkte with one
  sentence on what the star does. The star says what a click does (mark, or remove the
  Schwerpunkt). At most five stars: a sixth star is disabled and its tooltip says why, as
  does the star of a row without a competence; the count stands at the Schwerpunkte.
  Renaming or removing a starred competence takes its Schwerpunkt along. A file with more
  Schwerpunkte says that the first five were taken; one that does not count (no competence
  of that name) or a value that does not read is said there with "Wert entfernen". A value
  the backend refused marks its row. Enter goes to the next row, adds one after the last and
  ends the list on an empty last row (rows.ts); it never saves the profile.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import TextField from '$components/TextField.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import type { ProfileCompetence } from '$lib/ipc/types';
  import { MAX_FOCUS, type FieldProblem } from '$lib/state/profile.svelte';
  import { tick } from 'svelte';
  import NumberField from './NumberField.svelte';
  import { enterRow, focusRow } from './rows';
  import ValueNote from './ValueNote.svelte';

  interface Props {
    rows: ProfileCompetence[];
    focus: string[];
    /** Schwerpunkte of the file that do not count, or a value that does not read. */
    problems?: readonly FieldProblem[];
    /** How many Schwerpunkte the file named when more than five were taken over. */
    trimmed?: number | null;
    /** "Wert entfernen" of a value of `schwerpunkte` that does not read. */
    onclear?: () => void;
    /** A row the backend refused (its place among the rows with a name) and why. */
    error?: { row: number | null; text: string } | null;
  }

  let {
    rows = $bindable(),
    focus = $bindable(),
    problems = [],
    trimmed = null,
    onclear,
    error = null,
  }: Props = $props();

  const words = $derived(t.profile.field);
  const id = $props.id();
  let list = $state<HTMLElement | null>(null);

  const same = (a: string, b: string): boolean => a.trim().toLowerCase() === b.trim().toLowerCase();
  const starred = (name: string): boolean =>
    name.trim() !== '' && focus.some((entry) => same(entry, name));

  /** The row the backend refused: counted among the rows with a name, as the backend does. */
  const refused = $derived.by((): ProfileCompetence | null => {
    if (error === null || error.row === null) return null;
    return rows.filter((row) => row.name.trim() !== '')[error.row] ?? null;
  });

  function star(row: ProfileCompetence): void {
    const name = row.name.trim();
    if (name === '') return;
    if (starred(name)) {
      focus = focus.filter((entry) => !same(entry, name));
    } else if (focus.length < MAX_FOCUS) {
      focus = [...focus, name];
    }
  }

  function rename(row: ProfileCompetence, next: string): void {
    const old = row.name;
    row.name = next;
    if (!starred(old)) return;
    focus = focus
      .map((entry) => (same(entry, old) ? next.trim() : entry))
      .filter((entry) => entry !== '');
  }

  function remove(row: ProfileCompetence): void {
    const name = row.name;
    rows = rows.filter((other) => other !== row);
    if (starred(name)) focus = focus.filter((entry) => !same(entry, name));
  }

  const append = (): void => {
    rows = [...rows, { name: '', years: null, aliases: [], origin: null }];
  };

  async function add(): Promise<void> {
    append();
    await tick();
    focusRow(list, rows.length - 1);
  }

  const blank = (row: ProfileCompetence): boolean =>
    row.name.trim() === '' && row.years === null && row.aliases.length === 0;
  const enter = (row: ProfileCompetence): void =>
    void enterRow({ list, rows, row, blank, add: append, remove });

  /** "Wert entfernen" of one Schwerpunkt: it goes from the list at once. */
  function drop(problem: FieldProblem): void {
    if (problem.entry) focus = focus.filter((entry) => !same(entry, problem.value));
    else onclear?.();
  }
</script>

<div class="list" bind:this={list} data-testid="competences" data-field="competences">
  {#if rows.length > 0}
    <div class="head" aria-hidden="true">
      <span></span>
      <span>{words.competence}</span>
      <span use:tooltip={words.yearsHint}>{words.years}</span>
      <span class="aliases-head" use:tooltip={words.aliasesHint}>{words.aliases}</span>
      <span></span>
    </div>
  {/if}
  {#each rows as row, index (row)}
    {@const wrong = row === refused}
    <div
      class="row"
      data-row
      data-testid="competence-row"
      use:formKeys={{ save: () => enter(row) }}
    >
      <!-- Its words follow its state, like the favourite star of a job. -->
      <span class="star">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="star"
          label={starred(row.name) ? words.unstar : words.star}
          pressed={starred(row.name)}
          disabled={row.name.trim() === '' || (!starred(row.name) && focus.length >= MAX_FOCUS)}
          disabledReason={row.name.trim() === '' ? words.starEmpty : words.focusFull}
          testid="competence-star"
          onclick={() => star(row)}
        />
      </span>
      <span class="name">
        <TextField
          value={row.name}
          id="{id}-name-{index}"
          label={words.competence}
          placeholder={rows.length === 1 ? words.competencePlaceholder : null}
          invalid={wrong}
          describedby={wrong ? `${id}-error` : null}
          testid="competence-name"
          oninput={(next) => rename(row, next)}
        />
      </span>
      <span class="years">
        <NumberField
          bind:value={row.years}
          label={words.years}
          invalid={wrong}
          testid="competence-years"
        />
      </span>
      <span class="aliases">
        <ChipInput
          bind:values={row.aliases}
          label={words.aliases}
          placeholder={words.aliases}
          testid="competence-aliases"
        />
      </span>
      <span class="remove">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={words.removeCompetence(row.name.trim())}
          testid="competence-remove"
          onclick={() => remove(row)}
        />
      </span>
    </div>
  {/each}
  {#if error}
    <p class="error" id="{id}-error" role="alert" data-testid="competence-error">{error.text}</p>
  {/if}
  <span class="add" class:indent={rows.length > 0}>
    <Button
      variant="secondary"
      size="sm"
      icon="plus"
      label={words.addCompetence}
      testid="competence-add"
      onclick={() => void add()}
    />
  </span>
  <div class="focus" data-testid="focus" data-field="focus">
    <div class="focus-row">
      <span class="focus-label" data-testid="focus-count">
        {words.focusCount(focus.length, MAX_FOCUS)}
      </span>
      {#if focus.length > 0}
        <ChipInput bind:values={focus} entry={false} />
      {/if}
    </div>
    {#each problems as problem (problem.value)}
      <ValueNote
        text={problem.entry
          ? words.unreadableFocus(problem.value)
          : words.unreadableValue(problem.value)}
        testid="focus-unread"
        onremove={() => drop(problem)}
      />
    {/each}
    {#if trimmed !== null}
      <p class="focus-hint" data-testid="focus-trimmed">{words.focusTrimmed(trimmed)}</p>
    {/if}
    <p class="focus-hint">{words.focusHint}</p>
  </div>
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    container-type: inline-size;
  }

  /* star | competence | years | other terms | remove */
  .head,
  .row {
    display: grid;
    grid-template-columns:
      var(--control-sm) minmax(0, 5fr) calc(var(--space-64) + var(--space-8))
      minmax(0, 4fr) var(--control-sm);
    align-items: start;
    gap: var(--space-8);
  }

  .head {
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .star,
  .remove {
    display: flex;
    align-items: center;
    height: var(--control-md);
  }

  /* Under the rows the button lines up with the competence column; alone it starts at the
     card's edge. */
  .add.indent,
  .error {
    margin-left: calc(var(--control-sm) + var(--space-8));
  }

  .error {
    color: var(--danger-strong);
    font: var(--type-sm);
  }

  .focus {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    padding-top: var(--space-12);
    border-top: var(--border-width) solid var(--border);
  }

  .focus-row {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-height: var(--control-sm);
  }

  .focus-label {
    flex: none;
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .focus-hint {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  /* Narrow: the other terms go under the competence. */
  @container (width < 520px) {
    .head .aliases-head {
      display: none;
    }

    .row {
      grid-template-columns:
        var(--control-sm) minmax(0, 1fr) calc(var(--space-64) + var(--space-8))
        var(--control-sm);
    }

    .aliases {
      grid-column: 2 / 4;
      grid-row: 2;
    }
  }
</style>
