<!--
  The profile as a form, in the order a consultant thinks: person, competences and
  Schwerpunkte, experience, tools and certificates, languages, wishes (they only nudge the
  score) and the hard criteria (they exclude). A thin profile marks its empty sections. The
  save bar stays at the bottom of the view: "Speichern" (the one primary, only with a
  change) and "Verwerfen". Enter in a single-line field saves too.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import Field from '$components/Field.svelte';
  import Notice from '$components/Notice.svelte';
  import Segmented from '$components/Segmented.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import TextField from '$components/TextField.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { de } from '$lib/i18n/de';
  import { formKeys } from '$lib/input/input';
  import type { ProfileAvailability, ProfileQuality, RemoteWish } from '$lib/ipc/types';
  import { primaryFirst } from '$lib/platform';
  import { editor, isoDate } from '$lib/state/profile.svelte';
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
    onsave: () => void;
    ondiscard: () => void;
  }

  let { quality, busy, note, onsave, ondiscard }: Props = $props();

  const t = de.profile.field;
  const id = $props.id();
  const form = $derived(editor.after);
  const c = $derived(editor.after.criteria);
  const thin = $derived(quality === 'thin' || quality === 'empty');
  const actionFirst = primaryFirst();

  const COUNTRIES = Object.entries(de.profile.country).map(([code, label]) => ({
    id: code,
    label,
  }));
  /** Known countries, then codes of the profile the app does not name. */
  const countries = $derived([
    ...COUNTRIES,
    ...c.countries
      .filter((code) => !(code in de.profile.country))
      .map((code) => ({ id: code, label: code })),
  ]);

  const REMOTE: { id: RemoteWish; label: string }[] = (
    ['full', 'mostly', 'partly', 'onSite'] as const
  ).map((wish) => ({ id: wish, label: de.profile.remoteWish[wish] }));
  const AVAILABLE: { id: ProfileAvailability['kind']; label: string }[] = (
    ['unset', 'now', 'from'] as const
  ).map((kind) => ({ id: kind, label: de.profile.availability[kind] }));

  function setAvailable(kind: ProfileAvailability['kind']): void {
    c.available =
      kind === 'from'
        ? { kind, date: isoDate(editor.dateText) ?? editor.dateText.trim() }
        : { kind };
  }

  function setDate(text: string): void {
    editor.dateText = text;
    c.available = { kind: 'from', date: isoDate(text) ?? text.trim() };
  }

  let tried = $state(false);
  const dateError = $derived(
    editor.dateInvalid && (tried || editor.dateText.trim().length >= 8) ? t.dateInvalid : null,
  );

  function save(): void {
    if (!editor.dirty || busy) return;
    tried = true;
    if (editor.dateInvalid) return;
    onsave();
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

<div class="editor" use:formKeys={{ save }} data-testid="profile-form">
  <ProfileSection heading={de.profile.section.person} testid="section-person">
    <div class="pair">
      <Field label={t.name} for="{id}-name">
        <TextField id="{id}-name" bind:value={form.name} testid="profile-name-field" />
      </Field>
      <Field label={t.title} for="{id}-title">
        <TextField
          id="{id}-title"
          bind:value={form.title}
          placeholder={t.titlePlaceholder}
          testid="profile-title"
        />
      </Field>
    </div>
    <Field label={t.roles} for="{id}-roles">
      <ChipInput
        id="{id}-roles"
        bind:values={form.roles}
        placeholder={t.rolesPlaceholder}
        testid="profile-roles"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.competences}
    hint={quality && quality !== 'good' ? de.profile.qualityText[quality] : null}
    empty={thin && form.competences.length === 0}
    testid="section-competences"
  >
    <CompetenceList bind:rows={form.competences} bind:focus={form.focus} />
    <Field label={t.strengths} for="{id}-strengths">
      <ChipInput
        id="{id}-strengths"
        bind:values={form.strengths}
        placeholder={t.strengthsPlaceholder}
        testid="profile-strengths"
      />
    </Field>
    <Field label={t.keywords} for="{id}-keywords" hint={t.keywordsHint}>
      <ChipInput
        id="{id}-keywords"
        bind:values={form.keywords}
        placeholder={t.keywordsPlaceholder}
        describedby="{id}-keywords-message"
        testid="profile-keywords"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.experience}
    empty={thin && empty(form.years, form.degrees, form.industries)}
    testid="section-experience"
  >
    <div class="pair">
      <Field label={t.totalYears} for="{id}-years">
        <NumberField id="{id}-years" bind:value={form.years} testid="profile-years" />
      </Field>
    </div>
    <Field label={t.degrees} for="{id}-degrees">
      <ChipInput
        id="{id}-degrees"
        bind:values={form.degrees}
        placeholder={t.degreesPlaceholder}
        testid="profile-degrees"
      />
    </Field>
    <Field label={t.industries} for="{id}-industries">
      <ChipInput
        id="{id}-industries"
        bind:values={form.industries}
        placeholder={t.industriesPlaceholder}
        testid="profile-industries"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.tools}
    empty={thin && empty(form.tools, form.certificates)}
    testid="section-tools"
  >
    <Field label={t.tools} for="{id}-tools">
      <ChipInput
        id="{id}-tools"
        bind:values={form.tools}
        placeholder={t.toolsPlaceholder}
        testid="profile-tools"
      />
    </Field>
    <Field label={t.certificates} for="{id}-certificates">
      <ChipInput
        id="{id}-certificates"
        bind:values={form.certificates}
        placeholder={t.certificatesPlaceholder}
        testid="profile-certificates"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.languages}
    empty={thin && form.languages.length === 0}
    testid="section-languages"
  >
    <LanguageList bind:rows={form.languages} />
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.wishes}
    hint={de.profile.sectionHint.wishes}
    testid="section-wishes"
  >
    <div class="pair">
      <Field label={t.wishRate} for="{id}-wish-rate">
        <NumberField
          id="{id}-wish-rate"
          bind:value={form.wishes.dayRate}
          testid="profile-wish-rate"
        />
      </Field>
    </div>
    <div class="block">
      <span class="label">{t.remote}</span>
      <ChoiceButtons
        options={REMOTE}
        selected={form.wishes.remote === null ? [] : [form.wishes.remote]}
        label={t.remote}
        testid="profile-remote"
        onchange={(next) => (form.wishes.remote = (next[0] as RemoteWish | undefined) ?? null)}
      />
    </div>
    <Field label={t.regions} for="{id}-regions">
      <ChipInput
        id="{id}-regions"
        bind:values={form.wishes.regions}
        placeholder={t.regionsPlaceholder}
        testid="profile-regions"
      />
    </Field>
    <Field label={t.wishIndustries} for="{id}-wish-industries">
      <ChipInput
        id="{id}-wish-industries"
        bind:values={form.wishes.industries}
        placeholder={t.wishIndustriesPlaceholder}
        testid="profile-wish-industries"
      />
    </Field>
  </ProfileSection>

  <ProfileSection
    heading={de.profile.section.criteria}
    hint={de.profile.sectionHint.criteria}
    empty={noCriteria}
    testid="section-criteria"
  >
    <div class="pair">
      <Field label={t.minDayRate} for="{id}-min-rate">
        <NumberField id="{id}-min-rate" bind:value={c.minDayRate} testid="profile-min-rate" />
      </Field>
      <Field label={t.targetYears} for="{id}-target" hint={t.targetYearsHint}>
        <NumberField
          id="{id}-target"
          bind:value={c.targetYears}
          describedby="{id}-target-message"
          testid="profile-target-years"
        />
      </Field>
    </div>
    <div class="block">
      <span class="label">{t.countries}</span>
      <ChoiceButtons
        options={countries}
        selected={c.countries}
        label={t.countries}
        multiple
        testid="profile-countries"
        onchange={(next) => (c.countries = next)}
      />
    </div>
    <div class="block">
      <span class="label">{t.available}</span>
      <div class="available">
        <Segmented
          options={AVAILABLE}
          value={c.available.kind}
          label={t.available}
          size="sm"
          testid="profile-available"
          onchange={setAvailable}
        />
        {#if c.available.kind === 'from'}
          <span class="date">
            <TextField
              value={editor.dateText}
              label={t.date}
              placeholder={t.datePlaceholder}
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
      {/if}
    </div>
    <div class="toggles">
      <SettingRow label={t.remoteOutside} hint={t.remoteOutsideHint}>
        <Toggle
          checked={c.remoteOutside}
          label={t.remoteOutside}
          testid="profile-remote-outside"
          onchange={(on) => (c.remoteOutside = on)}
        />
      </SettingRow>
      <SettingRow label={t.noAnue}>
        <Toggle
          checked={c.noAnue}
          label={t.noAnue}
          testid="profile-no-anue"
          onchange={(on) => (c.noAnue = on)}
        />
      </SettingRow>
    </div>
    <h3 class="sub">{de.profile.section.permanent}</h3>
    <div class="pair">
      <Field label={t.minSalary} for="{id}-salary">
        <NumberField id="{id}-salary" bind:value={c.minSalary} testid="profile-min-salary" />
      </Field>
      <Field label={t.remoteMin} for="{id}-remote-min" hint={t.remoteMinHint}>
        <NumberField
          id="{id}-remote-min"
          bind:value={c.permanentRemoteMin}
          describedby="{id}-remote-min-message"
          testid="profile-remote-min"
        />
      </Field>
    </div>
    <Field label={t.places} for="{id}-places">
      <ChipInput
        id="{id}-places"
        bind:values={c.permanentPlaces}
        placeholder={t.placesPlaceholder}
        testid="profile-places"
      />
    </Field>
  </ProfileSection>
</div>

<div class="bar" data-testid="profile-save-bar">
  <div class="status">
    {#if note}
      <Notice tone="danger" variant="inline" text={note} testid="profile-save-error" />
    {/if}
  </div>
  <div class="buttons">
    {#snippet discard()}
      <Button
        variant="secondary"
        label={de.profile.discard}
        disabled={!editor.dirty || busy}
        testid="profile-discard"
        onclick={ondiscard}
      />
    {/snippet}
    {#if !actionFirst}{@render discard()}{/if}
    <Button
      variant="primary"
      label={de.profile.save}
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

  .sub {
    padding-top: var(--space-4);
    color: var(--text-label);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
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

  .buttons {
    display: flex;
    gap: var(--space-12);
  }
</style>
