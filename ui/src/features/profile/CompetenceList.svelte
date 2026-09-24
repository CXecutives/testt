<!--
  The core competences: one row each with the star (Schwerpunkt), the competence, its years
  and other words for it (`auch`), then "Kompetenz hinzufügen" and the Schwerpunkte. At most
  five stars; a sixth is refused with one short sentence. Renaming or removing a starred
  competence takes its Schwerpunkt along.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import Notice from '$components/Notice.svelte';
  import TextField from '$components/TextField.svelte';
  import { de } from '$lib/i18n/de';
  import type { ProfileCompetence } from '$lib/ipc/types';
  import { MAX_FOCUS } from '$lib/state/profile.svelte';
  import { tick } from 'svelte';
  import NumberField from './NumberField.svelte';

  interface Props {
    rows: ProfileCompetence[];
    focus: string[];
  }

  let { rows = $bindable(), focus = $bindable() }: Props = $props();

  const t = de.profile.field;
  const id = $props.id();
  let full = $state(false);
  let list = $state<HTMLElement | null>(null);

  const same = (a: string, b: string): boolean => a.trim().toLowerCase() === b.trim().toLowerCase();
  const starred = (name: string): boolean =>
    name.trim() !== '' && focus.some((entry) => same(entry, name));

  function star(row: ProfileCompetence): void {
    const name = row.name.trim();
    if (name === '') return;
    if (starred(name)) {
      focus = focus.filter((entry) => !same(entry, name));
      full = false;
    } else if (focus.length >= MAX_FOCUS) {
      full = true;
    } else {
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
    full = false;
  }

  async function add(): Promise<void> {
    rows = [...rows, { name: '', years: null, aliases: [], origin: null }];
    await tick();
    const added = [...(list?.querySelectorAll<HTMLElement>('[data-row]') ?? [])].at(-1);
    added?.querySelector('input')?.focus();
  }
</script>

<div class="list" bind:this={list} data-testid="competences">
  {#if rows.length > 0}
    <div class="head" aria-hidden="true">
      <span></span>
      <span>{t.competence}</span>
      <span>{t.years}</span>
      <span class="aliases-head">{t.aliases}</span>
      <span></span>
    </div>
  {/if}
  {#each rows as row, index (row)}
    <div class="row" data-row data-testid="competence-row">
      <span class="star">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="star"
          label={t.star}
          pressed={starred(row.name)}
          disabled={row.name.trim() === ''}
          testid="competence-star"
          onclick={() => star(row)}
        />
      </span>
      <span class="name">
        <TextField
          value={row.name}
          id="{id}-name-{index}"
          label={t.competence}
          placeholder={rows.length === 1 ? t.competencePlaceholder : null}
          testid="competence-name"
          oninput={(next) => rename(row, next)}
        />
      </span>
      <span class="years">
        <NumberField bind:value={row.years} label={t.years} testid="competence-years" />
      </span>
      <span class="aliases">
        <ChipInput
          bind:values={row.aliases}
          label={t.aliases}
          placeholder={rows.length === 1 ? t.aliasesPlaceholder : null}
          testid="competence-aliases"
        />
      </span>
      <span class="remove">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon="x"
          label={t.removeCompetence(row.name.trim())}
          testid="competence-remove"
          onclick={() => remove(row)}
        />
      </span>
    </div>
  {/each}
  <span class="add">
    <Button
      variant="ghost"
      size="sm"
      label={t.addCompetence}
      testid="competence-add"
      onclick={() => void add()}
    />
  </span>
  <div class="focus" data-testid="focus">
    <span class="focus-label">{t.focus}</span>
    {#if focus.length > 0}
      <ChipInput bind:values={focus} entry={false} onchange={() => (full = false)} />
    {:else}
      <span class="focus-hint">{t.focusHint}</span>
    {/if}
  </div>
  {#if full}
    <Notice tone="warning" variant="inline" text={t.focusFull} testid="focus-full" />
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    container-type: inline-size;
  }

  /* star | competence | years | other words | remove */
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
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .star,
  .remove {
    display: flex;
    align-items: center;
    height: var(--control-md);
  }

  .add {
    margin-left: calc(var(--control-sm) + var(--space-8) - var(--space-12));
  }

  .focus {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-height: var(--control-sm);
    padding-top: var(--space-12);
    border-top: var(--border-width) solid var(--border);
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

  /* Narrow: the other words go under the competence. */
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
