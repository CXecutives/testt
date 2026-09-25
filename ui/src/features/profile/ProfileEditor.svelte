<!--
  The profile as a form, in the order a consultant thinks: person, competences and
  Schwerpunkte, experience, tools and certificates, languages, wishes (they only nudge the
  score) and the hard criteria (they exclude). A thin profile marks its empty sections. The
  save bar stays at the bottom of the view: "Speichern" (the one primary, only with a
  change) and "Verwerfen", or Ctrl/Cmd+S. Enter never saves this long form: in the row
  lists it goes to the next row.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import Field from '$components/Field.svelte';
  import Notice from '$components/Notice.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import TextField from '$components/TextField.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import type { ProfileQuality, RemoteWish } from '$lib/ipc/types';
  import { primaryFirst } from '$lib/platform';
  import { editor, isoDate, type UnreadableField } from '$lib/state/profile.svelte';
  import ChoiceButtons from './ChoiceButtons.svelte';
  import CompetenceList from './CompetenceList.svelte';
  import LanguageList from './LanguageList.svelte';
  import NumberField from './NumberField.svelte';
  import ProfileSection from './ProfileSection.svelte';

  interface Props {
    /** How much the engine understands of what is shown (guidance for a thin profile). */
    quality: ProfileQuality | null;
    busy: boolean;
    /** A failure of the last save, in words. */
    note: string | null;
    /** The outcome of the last save (until the next change). */
    result: string | null;
    /** Values of the file the app could not read, by field (said there while it is empty). */
    unreadable: Partial<Record<UnreadableField, string>>;
    onsave: () => void;
    ondiscard: () => void;
  }

  let { quality, busy, note, result, unreadable, onsave, ondiscard }: Props = $props();

  const words = $derived(t.profile.field);
  const id = $props.id();
  const form = $derived(editor.after);
  const c = $derived(editor.after.criteria);

  /** What the file had at a field the app could not read, while the field is empty. */
  const unread = $derived({
    minSalary:
      unreadable.minSalary !== undefined && c.minSalary === null
        ? words.unreadableNumber(unreadable.minSalary)
        : null,
    remoteMin:
      unreadable.remoteMin !== undefined && c.permanentRemoteMin === null
        ? words.unreadableNumber(unreadable.remoteMin)
        : null,
    targetYears:
      unreadable.targetYears !== undefined && c.targetYears === null
        ? words.unreadableNumber(unreadable.targetYears)
        : null,
    places:
      unreadable.places !== undefined && c.permanentPlaces.length === 0
        ? words.unreadablePlaces(unreadable.places)
        : null,
    available:
      unreadable.available !== undefined && c.available.kind === 'unset'
        ? words.unreadableDate(unreadable.available)
        : null,
  });
  const thin = $derived(quality === 'thin' || quality === 'empty');
  const actionFirst = primaryFirst();

  /** Known countries, then codes of the profile the app does not name. */
  const countries = $derived([
    ...Object.entries(t.profile.country).map(([code, label]) => ({ id: code, label })),
    ...c.countries
      .filter((code) => !(code in t.profile.country))
      .map((code) => ({ id: code, label: code })),
  ]);

  const REMOTE = $derived<{ id: RemoteWish; label: string }[]>(
    (['full', 'mostly', 'partly', 'onSite'] as const).map((wish) => ({
      id: wish,
      label: t.profile.remoteWish[wish],
    })),
  );
  const AVAILABLE = $derived<{ id: 'now' | 'from'; label: string }[]>(
    (['now', 'from'] as const).map((kind) => ({
      id: kind,
      label: t.profile.availability[kind],
    })),
  );

  /** Nothing chosen is no availability; pressing the chosen one again clears it. */
  function setAvailable(chosen: string[]): void {
    const kind = chosen[0];
    c.available =
      kind === 'from'
        ? { kind, date: isoDate(editor.dateText) ?? editor.dateText.trim() }
        : kind === 'now'
          ? { kind }
          : { kind: 'unset' };
  }

  function setDate(text: string): void {
    editor.dateText = text;
    c.available = { kind: 'from', date: isoDate(text) ?? text.trim() };
  }

  let tried = $state(false);
  const dateError = $derived(
    editor.dateInvalid && (tried || editor.dateText.trim().length >= 8) ? words.dateInvalid : null,
  );

  /** Ready to save: a day that does not read is said at its field, which gets the caret. */
  export function ready(): boolean {
    tried = true;
    if (!editor.dateInvalid) return true;
    document.querySelector<HTMLInputElement>('[data-testid="profile-date"]')?.focus();
    return false;
  }

  let bar = $state<HTMLElement | null>(null);

  /** A focused control that ends under the sticky save bar moves up (WebKit does not
   *  apply scroll-margin when it scrolls a focused field into view). */
  function keepClear(event: FocusEvent): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    requestAnimationFrame(() => {
      const top = bar?.getBoundingClientRect().top ?? Infinity;
      if (target.isConnected && target.getBoundingClientRect().bottom > top) {
        target.scrollIntoView({ block: 'center' });
      }
    });
  }

  function save(): void {
    if (!editor.dirty || busy) return;
    if (ready()) onsave();
  }

  const empty = (...values: unknown[]): boolean =>
    values.every(
      (value) => value === null || value === '' || (Array.isArray(value) && value.length === 0),
    );
  const noCriteria = $derived(
    empty(c.minDayRate, c.countries, c.targetYears, c.minSalary, c.permanentPlaces) &&
      !c.noAnue &&
      c.available.kind === 'unset',
  );
</script>

<div
  class="editor"
  use:formKeys={{ shortcut: save }}
  onfocusin={keepClear}
  data-testid="profile-form"
>
  <ProfileSection heading={t.profile.section.person} testid="section-person">
    <div class="pair">
      <Field label={words.name} for="{id}-name">
        <TextField id="{id}-name" bind:value={form.name} testid="profile-name-field" />
      </Field>
      <Field label={words.title} for="{id}-title">
        <TextField
          id="{id}-title"
          bind:value={form.title}
          placeholder={words.titlePlaceholder}
          testid="profile-title"
        />
      </Field>
    </div>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.competences}
    hint={quality && quality !== 'good' ? t.profile.qualityText[quality] : null}
    empty={thin && form.competences.length === 0}
    testid="section-competences"
  >
    <CompetenceList bind:rows={form.competences} bind:focus={form.focus} />
    <Field label={words.strengths} for="{id}-strengths">
      <ChipInput
        id="{id}-strengths"
        bind:values={form.strengths}
        placeholder={words.strengthsPlaceholder}
        testid="profile-strengths"
      />
    </Field>
    <Field label={words.keywords} for="{id}-keywords" hint={words.keywordsHint}>
      <ChipInput
        id="{id}-keywords"
        bind:values={form.keywords}
        placeholder={words.keywordsPlaceholder}
        describedby="{id}-keywords-message"
        testid="profile-keywords"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.experience}
    empty={thin && empty(form.years, form.degrees, form.industries)}
    testid="section-experience"
  >
    <Field label={words.totalYears} for="{id}-years">
      <NumberField id="{id}-years" bind:value={form.years} testid="profile-years" />
    </Field>
    <Field label={words.degrees} for="{id}-degrees">
      <ChipInput
        id="{id}-degrees"
        bind:values={form.degrees}
        placeholder={words.degreesPlaceholder}
        testid="profile-degrees"
      />
    </Field>
    <Field label={words.industries} for="{id}-industries">
      <ChipInput
        id="{id}-industries"
        bind:values={form.industries}
        placeholder={words.industriesPlaceholder}
        testid="profile-industries"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.tools}
    empty={thin && empty(form.tools, form.certificates)}
    testid="section-tools"
  >
    <Field label={words.tools} for="{id}-tools">
      <ChipInput
        id="{id}-tools"
        bind:values={form.tools}
        placeholder={words.toolsPlaceholder}
        testid="profile-tools"
      />
    </Field>
    <Field label={words.certificates} for="{id}-certificates">
      <ChipInput
        id="{id}-certificates"
        bind:values={form.certificates}
        placeholder={words.certificatesPlaceholder}
        testid="profile-certificates"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.languages}
    empty={thin && form.languages.length === 0}
    testid="section-languages"
  >
    <LanguageList bind:rows={form.languages} />
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.wishes}
    hint={t.profile.sectionHint.wishes}
    testid="section-wishes"
  >
    <Field label={words.roles} for="{id}-roles">
      <ChipInput
        id="{id}-roles"
        bind:values={form.roles}
        placeholder={words.rolesPlaceholder}
        testid="profile-roles"
      />
    </Field>
    <Field label={words.wishRate} for="{id}-wish-rate">
      <NumberField
        id="{id}-wish-rate"
        money
        bind:value={form.wishes.dayRate}
        testid="profile-wish-rate"
      />
    </Field>
    <div class="block">
      <span class="label">{words.remote}</span>
      <ChoiceButtons
        options={REMOTE}
        selected={form.wishes.remote === null ? [] : [form.wishes.remote]}
        label={words.remote}
        testid="profile-remote"
        onchange={(next) => (form.wishes.remote = (next[0] as RemoteWish | undefined) ?? null)}
      />
    </div>
    <Field label={words.regions} for="{id}-regions">
      <ChipInput
        id="{id}-regions"
        bind:values={form.wishes.regions}
        placeholder={words.regionsPlaceholder}
        testid="profile-regions"
      />
    </Field>
    <Field label={words.wishIndustries} for="{id}-wish-industries">
      <ChipInput
        id="{id}-wish-industries"
        bind:values={form.wishes.industries}
        placeholder={words.wishIndustriesPlaceholder}
        testid="profile-wish-industries"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.criteria}
    hint={t.profile.sectionHint.criteria}
    empty={noCriteria}
    testid="section-criteria"
  >
    <div class="pair">
      <Field label={words.minDayRate} for="{id}-min-rate">
        <NumberField id="{id}-min-rate" money bind:value={c.minDayRate} testid="profile-min-rate" />
      </Field>
      <Field
        label={words.targetYears}
        for="{id}-target"
        hint={words.targetYearsHint}
        error={unread.targetYears}
      >
        <NumberField
          id="{id}-target"
          bind:value={c.targetYears}
          describedby="{id}-target-message"
          testid="profile-target-years"
        />
      </Field>
    </div>
    <div class="block">
      <span class="label">{words.countries}</span>
      <ChoiceButtons
        options={countries}
        selected={c.countries}
        label={words.countries}
        multiple
        testid="profile-countries"
        onchange={(next) => (c.countries = next)}
      />
    </div>
    <div class="block">
      <span class="label">{words.available}</span>
      <div class="available">
        <ChoiceButtons
          options={AVAILABLE}
          selected={c.available.kind === 'unset' ? [] : [c.available.kind]}
          label={words.available}
          testid="profile-available"
          onchange={setAvailable}
        />
        {#if c.available.kind === 'from'}
          <span class="date">
            <TextField
              value={editor.dateText}
              label={words.date}
              placeholder={words.datePlaceholder}
              invalid={dateError !== null}
              describedby="{id}-date-message"
              testid="profile-date"
              oninput={setDate}
            />
          </span>
        {/if}
      </div>
      {#if dateError}
        <Notice tone="danger" variant="inline" text={dateError} testid="profile-date-error" />
      {:else if unread.available}
        <Notice
          tone="danger"
          variant="inline"
          text={unread.available}
          testid="profile-available-unread"
        />
      {/if}
    </div>
    <div class="toggles">
      <SettingRow label={words.remoteOutside} for="{id}-remote-outside">
        <Toggle
          id="{id}-remote-outside"
          checked={c.remoteOutside}
          label={words.remoteOutside}
          testid="profile-remote-outside"
          onchange={(on) => (c.remoteOutside = on)}
        />
      </SettingRow>
      <SettingRow label={words.noAnue} for="{id}-no-anue">
        <Toggle
          id="{id}-no-anue"
          checked={c.noAnue}
          label={words.noAnue}
          testid="profile-no-anue"
          onchange={(on) => (c.noAnue = on)}
        />
      </SettingRow>
    </div>
    <h3 class="sub">{t.profile.section.permanent}</h3>
    <div class="pair">
      <Field label={words.minSalary} for="{id}-salary" error={unread.minSalary}>
        <NumberField id="{id}-salary" money bind:value={c.minSalary} testid="profile-min-salary" />
      </Field>
      <Field
        label={words.remoteMin}
        for="{id}-remote-min"
        hint={words.remoteMinHint}
        error={unread.remoteMin}
      >
        <NumberField
          id="{id}-remote-min"
          bind:value={c.permanentRemoteMin}
          describedby="{id}-remote-min-message"
          testid="profile-remote-min"
        />
      </Field>
    </div>
    <Field label={words.places} for="{id}-places" error={unread.places}>
      <ChipInput
        id="{id}-places"
        bind:values={c.permanentPlaces}
        placeholder={words.placesPlaceholder}
        testid="profile-places"
      />
    </Field>
  </ProfileSection>
</div>

<div class="bar" bind:this={bar} data-testid="profile-save-bar">
  <div class="status" data-testid="profile-save-status">
    {#if note}
      <Notice tone="danger" variant="inline" text={note} testid="profile-save-error" />
    {:else if tried && dateError}
      <Notice tone="danger" variant="inline" text={dateError} />
    {:else if editor.dirty}
      <span class="quiet">{t.profile.unsavedShort}</span>
    {:else if result}
      <Notice tone="success" variant="inline" text={result} testid="profile-saved" />
    {/if}
  </div>
  <div class="buttons">
    {#snippet discard()}
      <Button
        variant="secondary"
        label={t.profile.discard}
        disabled={!editor.dirty || busy}
        testid="profile-discard"
        onclick={ondiscard}
      />
    {/snippet}
    {#if !actionFirst}{@render discard()}{/if}
    <Button
      variant="primary"
      label={t.profile.save}
      disabled={!editor.dirty}
      loading={busy}
      testid="profile-save"
      onclick={save}
    />
    {#if actionFirst}{@render discard()}{/if}
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
  }

  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: start;
    gap: var(--space-16);
  }

  @container (width >= 520px) {
    .pair {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }

  .label {
    color: var(--text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .available {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  .date {
    width: calc(var(--stat-min) - var(--space-40));
  }

  .toggles {
    display: flex;
    flex-direction: column;
    border-top: var(--border-width) solid var(--border);
    border-bottom: var(--border-width) solid var(--border);
  }

  /* Festanstellung: its own group below the switches' hairline, a real subheading. */
  .sub {
    padding-top: var(--space-4);
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
  }

  /* The save bar stays in view at the bottom of the scrolling view. */
  .bar {
    position: sticky;
    z-index: var(--z-sticky);
    bottom: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12);
    margin-top: var(--space-24);
    padding: var(--space-12) 0;
    border-top: var(--border-width) solid var(--border);
    background-color: var(--surface);
  }

  .status {
    flex: 1;
    min-width: 0;
  }

  .quiet {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  /* Tab and focus scrolling keep a field clear of the sticky save bar. */
  .editor :global(:is(input, textarea, button, [role='switch'])) {
    scroll-margin-top: var(--pane-padding);
    scroll-margin-bottom: calc(var(--control-md) + 2 * var(--space-12) + var(--space-8));
  }

  .buttons {
    display: flex;
    gap: var(--space-12);
  }
</style>
