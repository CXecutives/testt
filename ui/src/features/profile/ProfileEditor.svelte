<!--
  The profile as a form, in the order a consultant thinks: person, competences and
  Schwerpunkte, experience and qualifications, languages, wishes (they only nudge the score),
  the exclusion criteria (they exclude), availability (it only marks) and at the end what the
  app reads in the file. Each block says in one sentence what it is for; only the
  competences are needed, which their sentence says once. Every field of a block has the
  height of a field (md), the toggle buttons too, and every number field one width with its
  unit beside it. The countries are a field that suggests the countries the engine knows
  (by their German and English names), with DACH in one click; a country of a file the app
  does not know stays as it is. The day of "Ab Datum" exists only while it is chosen and
  gets the caret when it is. A thin profile marks its empty sections. A value of the file the app
  could not read is said at its field with "Wert entfernen"; a value the backend refused is
  said there too, and the field gets the caret. The save bar stays at the bottom of the view:
  "Speichern" (the one primary, only with a change) and "Verwerfen", or Ctrl/Cmd+S; without a
  change both say why they wait. Enter never saves this long form: in the row lists it goes
  to the next row.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import Field from '$components/Field.svelte';
  import Notice from '$components/Notice.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import TextField from '$components/TextField.svelte';
  import Toggle from '$components/Toggle.svelte';
  import { de } from '$lib/i18n/de';
  import { en } from '$lib/i18n/en';
  import { t } from '$lib/i18n/t';
  import { formKeys } from '$lib/input/input';
  import type {
    Notice as NoticeData,
    ProfileQuality,
    ProfileUnderstanding,
    RemoteWish,
    UnreadableField,
  } from '$lib/ipc/types';
  import { primaryFirst } from '$lib/platform';
  import { editor, isoDate, type FieldError, type FieldProblem } from '$lib/state/profile.svelte';
  import { tick } from 'svelte';
  import ChoiceButtons from './ChoiceButtons.svelte';
  import CompetenceList from './CompetenceList.svelte';
  import LanguageList from './LanguageList.svelte';
  import NumberField from './NumberField.svelte';
  import ProfileReading from './ProfileReading.svelte';
  import ProfileSection from './ProfileSection.svelte';
  import ValueNote from './ValueNote.svelte';

  interface Props {
    /** How much the engine understands of the form as it is (guidance for a thin profile). */
    quality: ProfileQuality | null;
    /** Values of the file the app could not read that are still there. */
    problems: readonly FieldProblem[];
    /** The engine's warnings of the profile (Schwerpunkte taken over, a region rule). */
    warnings: readonly NoticeData[];
    /** What the app reads in the file ("So liest die App dein Profil"); `null` for a new one. */
    understood: ProfileUnderstanding | null;
    /** A value the backend refused on the last save. */
    fieldError: FieldError | null;
    busy: boolean;
    /** A failure of the last save, in words. */
    note: string | null;
    /** The outcome of the last save (until the next change). */
    result: string | null;
    /** The way on after the first save during setup (next to the result); `null` otherwise. */
    onnext: (() => void) | null;
    onsave: () => void;
    ondiscard: () => void;
  }

  let {
    quality,
    problems,
    warnings,
    understood,
    fieldError,
    busy,
    note,
    result,
    onnext,
    onsave,
    ondiscard,
  }: Props = $props();

  // Text typed into a chip field of the form counts as a change.
  editor.typed.share();

  const words = $derived(t.profile.field);
  const id = $props.id();
  const form = $derived(editor.after);
  const c = $derived(editor.after.criteria);

  /** The words of a value the app could not read, by the kind of its field. */
  function unreadText(problem: FieldProblem): string {
    switch (problem.field) {
      case 'minDayRate':
      case 'targetYears':
      case 'minSalary':
      case 'permanentRemoteMin':
      case 'wishDayRate':
        return words.unreadableNumber(problem.value);
      case 'available':
        return words.unreadableDate(problem.value);
      case 'roles':
        return problem.entry
          ? words.unreadableRole(problem.value)
          : words.unreadableValue(problem.value);
      default:
        return words.unreadableValue(problem.value);
    }
  }

  const problemsOf = (field: UnreadableField): FieldProblem[] =>
    problems.filter((problem) => problem.field === field);

  /** "Wert entfernen": an entry of a list goes at once, a whole value when saving. */
  function drop(problem: FieldProblem): void {
    const same = (entry: string): boolean => entry.toLowerCase() !== problem.value.toLowerCase();
    if (problem.entry && problem.field === 'roles') form.roles = form.roles.filter(same);
    else if (problem.entry && problem.field === 'focus') form.focus = form.focus.filter(same);
    else editor.clear(problem.field);
  }

  /** The error of a field: a value the backend refused, else the first value of the file
   *  that does not read (said by `Field` with "Wert entfernen" as its way on). */
  function errorOf(field: string): string | null {
    if (fieldError?.field === field) return fieldError.text();
    const first = problemsOf(field as UnreadableField).find((problem) => !problem.entry);
    return first ? unreadText(first) : null;
  }

  function removeOf(field: UnreadableField): {
    label: string;
    testid: string;
    onclick: () => void;
  } | null {
    if (fieldError?.field === field) return null;
    const first = problemsOf(field).find((problem) => !problem.entry);
    return first
      ? { label: words.removeValue, testid: 'value-remove', onclick: () => drop(first) }
      : null;
  }

  const listError = (field: string): { row: number | null; text: string } | null =>
    fieldError?.field === field ? { row: fieldError.row, text: fieldError.text() } : null;

  const trimmed = $derived.by((): number | null => {
    const notice = warnings.find((w) => w.code === 'focusTrimmed');
    return notice ? Number(notice.params.count) : null;
  });
  /** The minimum remote share of permanent roles without places (the rule stays off). */
  const regionWithoutPlaces = $derived(
    warnings.some((w) => w.code === 'regionWithoutPlaces') &&
      c.permanentPlaces.length === 0 &&
      c.permanentRemoteMin !== null,
  );

  const thin = $derived(quality === 'thin' || quality === 'empty');
  const actionFirst = primaryFirst();

  /** Every country the engine knows, named in the app's language and found by both names. */
  const COUNTRIES = $derived(
    Object.keys(de.profile.country).map((code) => ({
      id: code,
      label: t.profile.country[code] ?? code,
      terms: [de.profile.country[code] ?? code, en.profile.country[code] ?? code],
    })),
  );
  /** Deutschland, Österreich and Schweiz in one click. */
  const DACH = ['DE', 'AT', 'CH'];
  const dachMissing = $derived(DACH.some((code) => !c.countries.includes(code)));
  function addDach(): void {
    c.countries = [...c.countries, ...DACH.filter((code) => !c.countries.includes(code))];
  }

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

  /** Nothing chosen is no availability; pressing the chosen one again clears it. "Ab Datum"
   *  puts the caret into its day. */
  async function setAvailable(chosen: string[]): Promise<void> {
    const kind = chosen[0];
    c.available =
      kind === 'from'
        ? { kind, date: isoDate(editor.dateText) ?? editor.dateText.trim() }
        : kind === 'now'
          ? { kind }
          : { kind: 'unset' };
    if (kind !== 'from') return;
    await tick();
    root?.querySelector<HTMLInputElement>('[data-testid="profile-date"]')?.focus();
  }

  function setDate(text: string): void {
    editor.dateText = text;
    c.available = { kind: 'from', date: isoDate(text) ?? text.trim() };
  }

  let tried = $state(false);
  const dateError = $derived(
    editor.dateInvalid && (tried || editor.dateText.trim().length >= 8) ? words.dateInvalid : null,
  );

  let root = $state<HTMLElement | null>(null);

  /** Ready to save: a day that does not read is said at its field, which gets the caret. */
  export function ready(): boolean {
    tried = true;
    if (!editor.dateInvalid) return true;
    document.querySelector<HTMLInputElement>('[data-testid="profile-date"]')?.focus();
    return false;
  }

  /** The caret into the field a refused value belongs to (its marked control first), in
   *  the middle of the view; `false` when the field is not on the page. */
  export async function focusField(field: string): Promise<boolean> {
    await tick();
    const scope = root?.querySelector<HTMLElement>(`[data-field="${field}"]`);
    const target =
      scope?.querySelector<HTMLElement>('[aria-invalid="true"]') ??
      scope?.querySelector<HTMLElement>('input, textarea, button');
    target?.focus();
    target?.scrollIntoView({ block: 'center' });
    return target !== null && target !== undefined;
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

  /** The rules for permanent roles hide while those are excluded, unless a save refused one
   *  of their values: then they stay until the form is saved or discarded, so it can be put
   *  right. */
  const PERMANENT: readonly string[] = ['minSalary', 'permanentRemoteMin', 'permanentPlaces'];
  let permanentHeld = $state(false);
  $effect(() => {
    if (PERMANENT.includes(fieldError?.field ?? '')) permanentHeld = true;
    else if (!editor.dirty || !c.noPermanent) permanentHeld = false;
  });
  const permanentShown = $derived(!c.noPermanent || permanentHeld);

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
      !c.noPermanent,
  );
</script>

<div
  class="editor"
  bind:this={root}
  use:formKeys={{ shortcut: save }}
  onfocusin={keepClear}
  data-testid="profile-form"
>
  <ProfileSection
    heading={t.profile.section.person}
    hint={t.profile.sectionHint.person}
    testid="section-person"
  >
    <div class="pair">
      <div data-field="name">
        <Field label={words.name} for="{id}-name" error={errorOf('name')}>
          <TextField
            id="{id}-name"
            bind:value={form.name}
            placeholder={words.namePlaceholder}
            invalid={fieldError?.field === 'name'}
            describedby="{id}-name-message"
            testid="profile-name-field"
          />
        </Field>
      </div>
      <div data-field="title">
        <Field label={words.title} for="{id}-title" error={errorOf('title')}>
          <TextField
            id="{id}-title"
            bind:value={form.title}
            placeholder={words.titlePlaceholder}
            invalid={fieldError?.field === 'title'}
            describedby="{id}-title-message"
            testid="profile-title"
          />
        </Field>
      </div>
    </div>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.competences}
    hint={quality && quality !== 'good'
      ? t.profile.qualityText[quality]
      : t.profile.sectionHint.competences}
    empty={thin && form.competences.every((row) => row.name.trim() === '')}
    testid="section-competences"
  >
    <CompetenceList
      bind:rows={form.competences}
      bind:focus={form.focus}
      problems={problemsOf('focus')}
      {trimmed}
      onclear={() => editor.clear('focus')}
      error={listError('competences') ?? listError('focus')}
    />
    <div data-field="strengths">
      <Field
        label={words.strengths}
        for="{id}-strengths"
        hint={words.strengthsHint}
        error={errorOf('strengths')}
      >
        <ChipInput
          id="{id}-strengths"
          bind:values={form.strengths}
          split="lines"
          placeholder={words.strengthsPlaceholder}
          invalid={fieldError?.field === 'strengths'}
          describedby="{id}-strengths-message"
          testid="profile-strengths"
        />
      </Field>
    </div>
    <div data-field="keywords">
      <Field
        label={words.keywords}
        for="{id}-keywords"
        hint={words.keywordsHint}
        error={errorOf('keywords')}
      >
        <ChipInput
          id="{id}-keywords"
          bind:values={form.keywords}
          placeholder={words.keywordsPlaceholder}
          invalid={fieldError?.field === 'keywords'}
          describedby="{id}-keywords-message"
          testid="profile-keywords"
        />
      </Field>
    </div>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.experience}
    hint={t.profile.sectionHint.experience}
    empty={thin && empty(form.years, form.degrees, form.certificates, form.tools, form.industries)}
    testid="section-experience"
  >
    <div data-field="years">
      <Field
        label={words.totalYears}
        for="{id}-years"
        hint={words.totalYearsHint}
        error={errorOf('years')}
      >
        <NumberField
          id="{id}-years"
          unit={t.profile.unit.years}
          bind:value={form.years}
          invalid={fieldError?.field === 'years'}
          describedby="{id}-years-message"
          testid="profile-years"
        />
      </Field>
    </div>
    <div data-field="degrees">
      <Field label={words.degrees} for="{id}-degrees" error={errorOf('degrees')}>
        <ChipInput
          id="{id}-degrees"
          bind:values={form.degrees}
          split="lines"
          placeholder={words.degreesPlaceholder}
          invalid={fieldError?.field === 'degrees'}
          describedby="{id}-degrees-message"
          testid="profile-degrees"
        />
      </Field>
    </div>
    <div data-field="certificates">
      <Field label={words.certificates} for="{id}-certificates" error={errorOf('certificates')}>
        <ChipInput
          id="{id}-certificates"
          bind:values={form.certificates}
          split="lines"
          placeholder={words.certificatesPlaceholder}
          invalid={fieldError?.field === 'certificates'}
          describedby="{id}-certificates-message"
          testid="profile-certificates"
        />
      </Field>
    </div>
    <div data-field="tools">
      <Field label={words.tools} for="{id}-tools" error={errorOf('tools')}>
        <ChipInput
          id="{id}-tools"
          bind:values={form.tools}
          placeholder={words.toolsPlaceholder}
          invalid={fieldError?.field === 'tools'}
          describedby="{id}-tools-message"
          testid="profile-tools"
        />
      </Field>
    </div>
    <div data-field="industries">
      <Field label={words.industries} for="{id}-industries" error={errorOf('industries')}>
        <ChipInput
          id="{id}-industries"
          bind:values={form.industries}
          placeholder={words.industriesPlaceholder}
          invalid={fieldError?.field === 'industries'}
          describedby="{id}-industries-message"
          testid="profile-industries"
        />
      </Field>
    </div>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.languages}
    hint={t.profile.sectionHint.languages}
    empty={thin && form.languages.every((row) => row.language.trim() === '')}
    testid="section-languages"
  >
    <LanguageList bind:rows={form.languages} error={listError('languages')} />
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.wishes}
    hint={t.profile.sectionHint.wishes}
    testid="section-wishes"
  >
    <div data-field="roles">
      <Field
        label={words.roles}
        for="{id}-roles"
        hint={words.rolesHint}
        error={errorOf('roles')}
        action={removeOf('roles')}
      >
        <ChipInput
          id="{id}-roles"
          bind:values={form.roles}
          placeholder={words.rolesPlaceholder}
          invalid={errorOf('roles') !== null}
          describedby="{id}-roles-message"
          testid="profile-roles"
        />
      </Field>
      {#each problemsOf('roles').filter((problem) => problem.entry) as problem (problem.value)}
        <ValueNote
          text={unreadText(problem)}
          testid="roles-unread"
          onremove={() => drop(problem)}
        />
      {/each}
    </div>
    <div data-field="wishDayRate">
      <Field
        label={words.wishRate}
        for="{id}-wish-rate"
        hint={words.wishRateHint}
        error={errorOf('wishDayRate')}
        action={removeOf('wishDayRate')}
      >
        <NumberField
          id="{id}-wish-rate"
          money
          unit={t.profile.unit.euro}
          bind:value={form.wishes.dayRate}
          invalid={errorOf('wishDayRate') !== null}
          describedby="{id}-wish-rate-message"
          testid="profile-wish-rate"
        />
      </Field>
    </div>
    <div class="block" data-field="remote">
      <span class="label">{words.remote}</span>
      <ChoiceButtons
        options={REMOTE}
        selected={form.wishes.remote === null ? [] : [form.wishes.remote]}
        label={words.remote}
        testid="profile-remote"
        onchange={(next) => (form.wishes.remote = (next[0] as RemoteWish | undefined) ?? null)}
      />
      {#each problemsOf('remote') as problem (problem.value)}
        <ValueNote
          text={unreadText(problem)}
          testid="remote-unread"
          onremove={() => drop(problem)}
        />
      {/each}
    </div>
    <div data-field="regions">
      <Field
        label={words.regions}
        for="{id}-regions"
        error={errorOf('regions')}
        action={removeOf('regions')}
      >
        <ChipInput
          id="{id}-regions"
          bind:values={form.wishes.regions}
          placeholder={words.regionsPlaceholder}
          invalid={errorOf('regions') !== null}
          describedby="{id}-regions-message"
          testid="profile-regions"
        />
      </Field>
    </div>
    <div data-field="wishIndustries">
      <Field
        label={words.wishIndustries}
        for="{id}-wish-industries"
        error={errorOf('wishIndustries')}
        action={removeOf('wishIndustries')}
      >
        <ChipInput
          id="{id}-wish-industries"
          bind:values={form.wishes.industries}
          placeholder={words.wishIndustriesPlaceholder}
          invalid={errorOf('wishIndustries') !== null}
          describedby="{id}-wish-industries-message"
          testid="profile-wish-industries"
        />
      </Field>
    </div>
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.criteria}
    hint={t.profile.sectionHint.criteria}
    empty={noCriteria}
    testid="section-criteria"
  >
    <div class="pair">
      <div data-field="minDayRate">
        <Field
          label={words.minDayRate}
          for="{id}-min-rate"
          hint={words.minDayRateHint}
          error={errorOf('minDayRate')}
          action={removeOf('minDayRate')}
        >
          <NumberField
            id="{id}-min-rate"
            money
            unit={t.profile.unit.euro}
            bind:value={c.minDayRate}
            invalid={errorOf('minDayRate') !== null}
            describedby="{id}-min-rate-message"
            testid="profile-min-rate"
          />
        </Field>
      </div>
      <div data-field="targetYears">
        <Field
          label={words.targetYears}
          for="{id}-target"
          hint={words.targetYearsHint}
          error={errorOf('targetYears')}
          action={removeOf('targetYears')}
        >
          <NumberField
            id="{id}-target"
            unit={t.profile.unit.years}
            bind:value={c.targetYears}
            invalid={errorOf('targetYears') !== null}
            describedby="{id}-target-message"
            testid="profile-target-years"
          />
        </Field>
      </div>
    </div>
    <div data-field="countries">
      <Field
        label={words.countries}
        for="{id}-countries"
        error={errorOf('countries')}
        action={removeOf('countries')}
      >
        <div class="countries">
          <ChipInput
            id="{id}-countries"
            bind:values={c.countries}
            options={COUNTRIES}
            noMatch={words.countryNone}
            placeholder={words.countriesPlaceholder}
            invalid={errorOf('countries') !== null}
            describedby="{id}-countries-message"
            testid="profile-countries"
          />
          {#if dachMissing}
            <Button
              variant="secondary"
              icon="plus"
              label={words.dach}
              testid="profile-dach"
              onclick={addDach}
            />
          {/if}
        </div>
      </Field>
    </div>
    <div class="toggles">
      <div data-field="remoteOutside">
        <SettingRow
          label={words.remoteOutside}
          hint={words.remoteOutsideHint}
          for="{id}-remote-outside"
        >
          <Toggle
            id="{id}-remote-outside"
            checked={c.remoteOutside}
            label={words.remoteOutside}
            disabled={c.countries.length === 0}
            disabledReason={words.remoteOutsideOff}
            testid="profile-remote-outside"
            onchange={(on) => (c.remoteOutside = on)}
          />
        </SettingRow>
        {#each problemsOf('remoteOutside') as problem (problem.value)}
          <ValueNote
            text={unreadText(problem)}
            testid="remote-outside-unread"
            onremove={() => drop(problem)}
          />
        {/each}
      </div>
      <div data-field="contracts">
        <SettingRow label={words.noAnue} for="{id}-no-anue">
          <Toggle
            id="{id}-no-anue"
            checked={c.noAnue}
            label={words.noAnue}
            testid="profile-no-anue"
            onchange={(on) => (c.noAnue = on)}
          />
        </SettingRow>
        <SettingRow label={words.noPermanent} hint={words.noPermanentHint} for="{id}-no-permanent">
          <Toggle
            id="{id}-no-permanent"
            checked={c.noPermanent}
            label={words.noPermanent}
            testid="profile-no-permanent"
            onchange={(on) => (c.noPermanent = on)}
          />
        </SettingRow>
        {#each problemsOf('contracts') as problem (problem.value)}
          <ValueNote
            text={unreadText(problem)}
            testid="contracts-unread"
            onremove={() => drop(problem)}
          />
        {/each}
      </div>
    </div>
    {#if permanentShown}
      <div class="sub" data-testid="profile-permanent">
        <h3 class="sub-heading">{t.profile.section.permanent}</h3>
        <p class="sub-hint">{t.profile.sectionHint.permanent}</p>
      </div>
      <div class="pair">
        <div data-field="minSalary">
          <Field
            label={words.minSalary}
            for="{id}-salary"
            error={errorOf('minSalary')}
            action={removeOf('minSalary')}
          >
            <NumberField
              id="{id}-salary"
              money
              unit={t.profile.unit.euro}
              bind:value={c.minSalary}
              invalid={errorOf('minSalary') !== null}
              describedby="{id}-salary-message"
              testid="profile-min-salary"
            />
          </Field>
        </div>
        <div data-field="permanentRemoteMin">
          <Field
            label={words.remoteMin}
            for="{id}-remote-min"
            hint={words.remoteMinHint}
            error={errorOf('permanentRemoteMin') ??
              (regionWithoutPlaces ? t.profile.warning.regionWithoutPlaces : null)}
            action={removeOf('permanentRemoteMin')}
          >
            <NumberField
              id="{id}-remote-min"
              unit={t.profile.unit.percent}
              bind:value={c.permanentRemoteMin}
              invalid={errorOf('permanentRemoteMin') !== null || regionWithoutPlaces}
              describedby="{id}-remote-min-message"
              testid="profile-remote-min"
            />
          </Field>
        </div>
      </div>
      <div data-field="permanentPlaces">
        <Field
          label={words.places}
          for="{id}-places"
          error={errorOf('permanentPlaces')}
          action={removeOf('permanentPlaces')}
        >
          <ChipInput
            id="{id}-places"
            bind:values={c.permanentPlaces}
            invalid={errorOf('permanentPlaces') !== null}
            describedby="{id}-places-message"
            placeholder={words.placesPlaceholder}
            testid="profile-places"
          />
        </Field>
      </div>
    {/if}
  </ProfileSection>

  <ProfileSection
    heading={t.profile.section.availability}
    hint={t.profile.sectionHint.availability}
    testid="section-availability"
  >
    <div class="block" data-field="available">
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
              invalid={dateError !== null || fieldError?.field === 'available'}
              describedby="{id}-date-message"
              testid="profile-date"
              oninput={setDate}
            />
          </span>
        {/if}
      </div>
      {#if dateError}
        <Notice tone="danger" variant="inline" text={dateError} testid="profile-date-error" />
      {:else if fieldError?.field === 'available'}
        <Notice tone="danger" variant="inline" text={fieldError.text()} />
      {/if}
      {#each problemsOf('available') as problem (problem.value)}
        <ValueNote
          text={unreadText(problem)}
          testid="profile-available-unread"
          onremove={() => drop(problem)}
        />
      {/each}
    </div>
  </ProfileSection>

  {#if understood}
    <ProfileReading {understood} stale={editor.dirty} />
  {/if}
</div>

<div class="bar" bind:this={bar} data-testid="profile-save-bar">
  <div class="status" data-testid="profile-save-status">
    {#if note}
      <Notice tone="danger" variant="inline" text={note} testid="profile-save-error" />
    {:else if tried && dateError}
      <Notice tone="danger" variant="inline" text={dateError} />
    {:else if editor.dirty}
      <span class="quiet">{t.profile.unsaved}</span>
    {:else if result}
      <span class="result">
        <Notice tone="success" variant="inline" text={result} testid="profile-saved" />
        {#if onnext}
          <Button
            variant="secondary"
            icon="arrow-right"
            label={t.profile.next}
            testid="profile-next"
            onclick={onnext}
          />
        {/if}
      </span>
    {/if}
  </div>
  <div class="buttons">
    {#snippet discard()}
      <Button
        variant="secondary"
        label={t.profile.discard}
        disabled={!editor.dirty || busy}
        disabledReason={editor.dirty ? null : t.profile.noChanges}
        testid="profile-discard"
        onclick={ondiscard}
      />
    {/snippet}
    {#if !actionFirst}{@render discard()}{/if}
    <Button
      variant="primary"
      label={t.profile.save}
      disabled={!editor.dirty}
      disabledReason={t.profile.noChanges}
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

  /* The day is as wide as every number field. */
  .date {
    width: calc(var(--stat-min) - var(--space-48));
  }

  /* Narrow, DACH sits under the field, which keeps the full width of its neighbours. */
  .countries {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-12);
  }

  @container (width >= 520px) {
    .countries {
      flex-direction: row;
    }
  }

  .countries > :global(:first-child) {
    flex: 1;
    align-self: stretch;
    min-width: 0;
  }

  .toggles {
    display: flex;
    flex-direction: column;
    border-top: var(--border-width) solid var(--border);
    border-bottom: var(--border-width) solid var(--border);
  }

  /* Festanstellung: its own group below the switches' hairline, a real subheading. */
  .sub {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding-top: var(--space-4);
  }

  .sub-heading {
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
  }

  .sub-hint {
    color: var(--text-muted);
    font: var(--type-sm);
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

  .result {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-8) var(--space-16);
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
