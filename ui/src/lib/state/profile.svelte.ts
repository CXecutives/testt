// The profile editor (Profil view): the form as it was handed out (`before`), the form as
// the user has it (`after`) and where it came from. Saving sends both; the backend writes
// only what differs and keeps every other key of the file. A draft (a new profile, a chosen
// file, an AI's answer) is unsaved until it is saved; the stored profile only once
// something differs. Leaving the view or closing the window with unsaved changes asks first
// (ProfileView holds the guard and the dialog).
//
// Values of the file the engine could not read are said at their field (`fieldProblems`);
// "Wert entfernen" clears one (`clear`), saving then removes its keys. Quality and the empty
// sections follow the form while typing (`localQuality`, with the engine's thresholds).

import { language } from '../i18n/language.svelte';
import { invoke } from '../ipc/api';
import type {
  Notice,
  ProfileCompetence,
  ProfileDraft,
  ProfileForm,
  ProfileInfo,
  ProfileLanguage,
  ProfileQuality,
  ProfileUnderstanding,
  UnreadableField,
} from '../ipc/types';
import { TypedText } from './typed.svelte';

/** Where the form in the editor came from: the stored profile, a new one, a chosen file, an
 *  AI's answer for a new profile, or an answer that updates the stored profile. */
export type DraftOrigin = 'stored' | 'new' | 'file' | 'answer' | 'update';

/** At most this many competences are Schwerpunkte (the backend refuses more). */
export const MAX_FOCUS = 5;

/** Fewer terms than this make a thin profile (the engine's `THIN_BELOW`). */
const THIN_BELOW = 5;

/** The JSON a new profile is written into. */
const NEW_SOURCE = '{}';

export function emptyForm(): ProfileForm {
  return {
    name: '',
    title: '',
    competences: [],
    strengths: [],
    keywords: [],
    years: null,
    degrees: [],
    industries: [],
    tools: [],
    certificates: [],
    languages: [],
    focus: [],
    roles: [],
    wishes: { dayRate: null, remote: null, regions: [], industries: [] },
    criteria: {
      minDayRate: null,
      countries: [],
      noAnue: false,
      noPermanent: false,
      available: { kind: 'unset' },
      // Missing in the file means allowed, as the engine reads it.
      remoteOutside: true,
      targetYears: null,
      minSalary: null,
      permanentPlaces: [],
      permanentRemoteMin: null,
    },
  };
}

const same = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

/** Trimmed, without empty entries, each entry once (case-insensitive). */
export function cleanList(items: readonly string[]): string[] {
  const out: string[] = [];
  for (const item of items) {
    const text = item.trim();
    if (text !== '' && !out.some((o) => same(o, text))) out.push(text);
  }
  return out;
}

const positive = (value: number | null): number | null =>
  value !== null && value > 0 ? value : null;

/** The form the way the backend compares it (trimmed, empty rows and entries gone). */
export function normalized(form: ProfileForm): ProfileForm {
  const c = form.criteria;
  const competences: ProfileCompetence[] = form.competences
    .filter((row) => row.name.trim() !== '')
    .map((row) => ({ ...row, name: row.name.trim(), aliases: cleanList(row.aliases) }));
  const languages: ProfileLanguage[] = form.languages
    .filter((row) => row.language.trim() !== '')
    .map((row) => ({ ...row, language: row.language.trim() }));
  return {
    name: form.name.trim(),
    title: form.title.trim(),
    competences,
    strengths: cleanList(form.strengths),
    keywords: cleanList(form.keywords),
    years: form.years,
    degrees: cleanList(form.degrees),
    industries: cleanList(form.industries),
    tools: cleanList(form.tools),
    certificates: cleanList(form.certificates),
    languages,
    focus: cleanList(form.focus),
    roles: cleanList(form.roles),
    wishes: {
      dayRate: positive(form.wishes.dayRate),
      remote: form.wishes.remote,
      regions: cleanList(form.wishes.regions),
      industries: cleanList(form.wishes.industries),
    },
    criteria: {
      minDayRate: positive(c.minDayRate),
      countries: cleanList(c.countries.map((code) => code.toUpperCase())),
      noAnue: c.noAnue,
      noPermanent: c.noPermanent,
      available:
        c.available.kind === 'from' ? { kind: 'from', date: c.available.date.trim() } : c.available,
      remoteOutside: c.remoteOutside,
      targetYears: positive(c.targetYears),
      minSalary: positive(c.minSalary),
      permanentPlaces: cleanList(c.permanentPlaces),
      permanentRemoteMin: positive(c.permanentRemoteMin),
    },
  };
}

export function sameForm(a: ProfileForm, b: ProfileForm): boolean {
  return JSON.stringify(normalized(a)) === JSON.stringify(normalized(b));
}

const copy = (form: ProfileForm): ProfileForm => structuredClone($state.snapshot(form));

// ------------------------------------------------------------------ quality

/** The texts of the form the engine counts as terms for the quality: the role, the
 *  competences, the lists of the experience and the strengths and keywords (its other words
 *  and wishes do not count). */
function terms(form: ProfileForm): number {
  const n = normalized(form);
  return cleanList([
    n.title,
    ...n.competences.map((row) => row.name),
    ...n.strengths,
    ...n.keywords,
    ...n.degrees,
    ...n.industries,
    ...n.tools,
    ...n.certificates,
    ...n.languages.map((row) => row.language),
  ]).length;
}

/**
 * The quality of the form as the user has it, with the engine's thresholds (no terms empty,
 * fewer than five thin): the engine's count for what the form started with (it also counts
 * what only the file holds, the career stations), moved by what the form changed.
 */
export function localQuality(
  counted: number,
  before: ProfileForm,
  after: ProfileForm,
): { quality: ProfileQuality; terms: number } {
  const count = Math.max(0, counted - terms(before) + terms(after));
  const quality: ProfileQuality = count === 0 ? 'empty' : count < THIN_BELOW ? 'thin' : 'good';
  return { quality, terms: count };
}

// ------------------------------------------------------------------ values that did not read

/** A value the backend refused on saving: the field (and row) it names and its words (said
 *  when they show, so they follow a switch of the language). */
export interface FieldError {
  field: string;
  row: number | null;
  text: () => string;
}

/** A value of the file the engine could not read, at the field of the form that holds it. */
export interface FieldProblem {
  field: UnreadableField;
  /** The engine's warning (the head names it in its tooltip). */
  notice: Notice;
  /** The value as the file had it (a JSON text loses its quotes). */
  value: string;
  /** One entry of a list does not count (a Schwerpunkt that is no competence, a target role
   *  without a field), not the whole value: removing it is removing the entry. */
  entry: boolean;
}

const FIELDS: readonly UnreadableField[] = [
  'minDayRate',
  'countries',
  'contracts',
  'remoteOutside',
  'available',
  'targetYears',
  'minSalary',
  'permanentPlaces',
  'permanentRemoteMin',
  'focus',
  'roles',
  'wishDayRate',
  'remote',
  'regions',
  'wishIndustries',
];

const isField = (value: unknown): value is UnreadableField =>
  typeof value === 'string' && (FIELDS as readonly string[]).includes(value);

/** The value of a field of the form, to see whether the user changed it. */
function fieldValue(form: ProfileForm, field: UnreadableField): unknown {
  const c = form.criteria;
  const w = form.wishes;
  switch (field) {
    case 'minDayRate':
      return c.minDayRate;
    case 'countries':
      return c.countries;
    case 'contracts':
      return [c.noAnue, c.noPermanent];
    case 'remoteOutside':
      return c.remoteOutside;
    case 'available':
      return c.available;
    case 'targetYears':
      return c.targetYears;
    case 'minSalary':
      return c.minSalary;
    case 'permanentPlaces':
      return c.permanentPlaces;
    case 'permanentRemoteMin':
      return c.permanentRemoteMin;
    case 'focus':
      return form.focus;
    case 'roles':
      return form.roles;
    case 'wishDayRate':
      return w.dayRate;
    case 'remote':
      return w.remote;
    case 'regions':
      return w.regions;
    case 'wishIndustries':
      return w.industries;
  }
}

const text = (value: unknown): string =>
  typeof value === 'string' ? value.replace(/^"(.*)"$/, '$1') : String(value ?? '');

/**
 * The values of the file the engine could not read that are still there: not removed with
 * "Wert entfernen" (`cleared`) and not replaced by a new value in the form. A Schwerpunkt or a
 * target role that does not count is fixed once it is gone from its list (or, for a
 * Schwerpunkt, once a competence carries its name).
 */
export function fieldProblems(
  warnings: readonly Notice[],
  before: ProfileForm,
  after: ProfileForm,
  cleared: readonly UnreadableField[],
): FieldProblem[] {
  const out: FieldProblem[] = [];
  for (const notice of warnings) {
    const field = notice.code === 'availabilityNotUnderstood' ? 'available' : notice.params.field;
    if (!isField(field) || cleared.includes(field)) continue;
    const value = text(notice.params.value);
    const list = field === 'focus' ? before.focus : field === 'roles' ? before.roles : [];
    const entry = list.some((item) => same(item, value));
    if (entry) {
      const now = field === 'focus' ? after.focus : after.roles;
      const named =
        field === 'focus' && after.competences.some((row) => same(row.name.trim(), value));
      if (!now.some((item) => same(item, value)) || named) continue;
    } else if (
      JSON.stringify(fieldValue(before, field)) !== JSON.stringify(fieldValue(after, field))
    ) {
      continue;
    }
    out.push({ field, notice, value, entry });
  }
  return out;
}

// ------------------------------------------------------------------ dates

const pad = (value: number): string => String(value).padStart(2, '0');

/** `2026-11-01` -> `01.11.2026`, in English `01/11/2026` (as the field shows a day). */
export function shownDate(iso: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  const mark = language.current === 'de' ? '.' : '/';
  return match ? `${match[3]}${mark}${match[2]}${mark}${match[1]}` : iso;
}

const GERMAN_DAY = /^(\d{1,2})[./](\d{1,2})[./](\d{2}|\d{4})$/;
const ISO_DAY = /^(\d{4})-(\d{1,2})-(\d{1,2})$/;

/**
 * A typed day (`1.11.2026`, `01.11.26`, `01/11/2026`, `2026-11-01`) as `YYYY-MM-DD`; `null`
 * if it is none. Day first in both languages (German and British English).
 */
export function isoDate(text: string): string | null {
  const value = text.trim();
  const german = GERMAN_DAY.exec(value);
  const iso = ISO_DAY.exec(value);
  const [year, month, day] = german
    ? [
        Number(german[3]) + (german[3]!.length === 2 ? 2000 : 0),
        Number(german[2]),
        Number(german[1]),
      ]
    : iso
      ? [Number(iso[1]), Number(iso[2]), Number(iso[3])]
      : [0, 0, 0];
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1] ?? 0;
  const valid = year > 1900 && day >= 1 && day <= days;
  return valid ? `${year}-${pad(month)}-${pad(day)}` : null;
}

/** A typed day in a form `isoDate` reads, whether or not the calendar has it (`31.02.2026`). */
export function dayShaped(text: string): boolean {
  const value = text.trim();
  return GERMAN_DAY.test(value) || ISO_DAY.test(value);
}

/** The day field's text for a form. */
const dateTextOf = (form: ProfileForm): string =>
  form.criteria.available.kind === 'from' ? shownDate(form.criteria.available.date) : '';

// ------------------------------------------------------------------ update from a CV

/** Entries of both lists, the stored ones first, each once. */
const union = (stored: readonly string[], added: readonly string[]): string[] =>
  cleanList([...stored, ...added]);

/**
 * The stored profile updated with an AI's answer to a CV: the answer fills and adds (a role,
 * years, new competences and other terms, new list entries, languages and their levels), the
 * stored profile keeps everything the answer leaves out, the Schwerpunkte and the user's
 * wishes and criteria (the answer only fills what the profile leaves empty). Rows keep their
 * place in the stored file (`origin`); new rows have none.
 */
export function updated(stored: ProfileForm, answer: ProfileForm): ProfileForm {
  const a = normalized(answer);
  const form: ProfileForm = {
    ...structuredClone(stored),
    name: stored.name.trim() || a.name,
    title: a.title || stored.title,
    years: a.years ?? stored.years,
  };
  for (const row of a.competences) {
    const match = form.competences.find((own) => same(own.name.trim(), row.name));
    if (match) {
      match.years = row.years ?? match.years;
      match.aliases = union(match.aliases, row.aliases);
    } else {
      form.competences.push({ ...row, origin: null });
    }
  }
  form.strengths = union(form.strengths, a.strengths);
  form.keywords = union(form.keywords, a.keywords);
  form.degrees = union(form.degrees, a.degrees);
  form.industries = union(form.industries, a.industries);
  form.tools = union(form.tools, a.tools);
  form.certificates = union(form.certificates, a.certificates);
  for (const row of a.languages) {
    const match = form.languages.find((own) => same(own.language.trim(), row.language));
    if (match) match.level = row.level ?? match.level;
    else form.languages.push({ ...row, origin: null });
  }
  if (form.focus.length === 0) form.focus = a.focus.slice(0, MAX_FOCUS);
  form.roles = union(form.roles, a.roles);
  const w = form.wishes;
  w.dayRate ??= a.wishes.dayRate;
  w.remote ??= a.wishes.remote;
  w.regions = union(w.regions, a.wishes.regions);
  w.industries = union(w.industries, a.wishes.industries);
  const c = form.criteria;
  const ac = a.criteria;
  c.minDayRate ??= ac.minDayRate;
  if (c.countries.length === 0) c.countries = ac.countries;
  c.noAnue ||= ac.noAnue;
  c.noPermanent ||= ac.noPermanent;
  if (c.available.kind === 'unset') c.available = ac.available;
  c.targetYears ??= ac.targetYears;
  c.minSalary ??= ac.minSalary;
  if (c.permanentPlaces.length === 0) c.permanentPlaces = ac.permanentPlaces;
  c.permanentRemoteMin ??= ac.permanentRemoteMin;
  return form;
}

// ------------------------------------------------------------------ the editor

class ProfileEditor {
  /** `null`: nothing in the editor (no profile yet, or the view has not opened one). */
  origin = $state<DraftOrigin | null>(null);
  before = $state.raw<ProfileForm>(emptyForm());
  after = $state<ProfileForm>(emptyForm());
  source = $state<string | null>(null);
  /** How much the engine understands of a draft (the stored profile has its own). */
  quality = $state<ProfileQuality | null>(null);
  /** What the engine reads in a draft from a file or an answer (its warnings, its terms). */
  understood = $state.raw<ProfileUnderstanding | null>(null);
  /** Values of the file the user removed ("Wert entfernen"); saving removes their keys. */
  cleared = $state<UnreadableField[]>([]);
  /** The steps to fill the profile from a CV with an AI are open. */
  pasting = $state(false);
  /** The AI's answer as pasted: kept until it fills the form, also when the steps close or
   *  the view changes. */
  answer = $state('');
  /** The day of "Verfügbar ab" as typed (the form holds it as `YYYY-MM-DD`). */
  dateText = $state('');
  /** Text typed into a chip field that is no chip yet: a change like any other. */
  readonly typed = new TypedText();

  get dirty(): boolean {
    if (this.origin === null) return false;
    if (this.origin === 'file' || this.origin === 'answer' || this.origin === 'update') {
      return true;
    }
    return this.cleared.length > 0 || this.typed.any || !sameForm(this.before, this.after);
  }

  #start(origin: DraftOrigin, before: ProfileForm, after: ProfileForm): void {
    this.origin = origin;
    this.before = copy(before);
    this.after = copy(after);
    this.dateText = dateTextOf(after);
    this.cleared = [];
    this.pasting = false;
  }

  /** The stored profile (again, e.g. after a save). */
  edit(form: ProfileForm): void {
    this.#start('stored', form, form);
    this.source = null;
    this.quality = null;
    this.understood = null;
  }

  /** An empty form for a new profile, with one empty competence and language row, so the
   *  table and the star show at once. */
  create(): void {
    this.#start('new', emptyForm(), {
      ...emptyForm(),
      competences: [{ name: '', years: null, aliases: [], origin: null }],
      languages: [{ language: '', level: null, origin: null }],
    });
    this.source = NEW_SOURCE;
    this.quality = null;
    this.understood = null;
  }

  /** A chosen file or an AI's answer, to review before it is saved. */
  take(draft: ProfileDraft, origin: 'file' | 'answer'): void {
    this.#start(origin, draft.form, draft.form);
    this.source = draft.source;
    this.quality = draft.quality;
    this.understood = draft.understood;
  }

  /** An AI's answer that updates the stored profile: saving merges it into the stored file,
   *  which the draft brings with the answer's career stations (they are no field of the form). */
  update(draft: ProfileDraft, stored: ProfileForm): void {
    this.#start('update', stored, updated(copy(stored), copy(draft.form)));
    this.source = draft.source;
    this.quality = null;
    this.understood = null;
  }

  /** Nothing in the editor (the empty state shows). */
  close(): void {
    this.#start('stored', emptyForm(), emptyForm());
    this.origin = null;
    this.source = null;
    this.quality = null;
    this.understood = null;
  }

  /** "Wert entfernen": the value of the file goes when the profile is saved. */
  clear(field: UnreadableField): void {
    if (!this.cleared.includes(field)) this.cleared = [...this.cleared, field];
  }

  /** "Ab Datum" is chosen but the day does not read. */
  get dateInvalid(): boolean {
    return this.after.criteria.available.kind === 'from' && isoDate(this.dateText) === null;
  }

  /** Drops the changes: the stored profile as saved, or no draft at all. */
  discard(stored: ProfileForm | null): void {
    this.typed.clear();
    if (stored !== null) this.edit(stored);
    else this.close();
  }

  /** Writes the form; the caller reloads the app state (and with it the stored form). */
  save(): Promise<ProfileInfo> {
    return invoke('save_profile', {
      save: {
        before: this.before,
        after: normalized(copy(this.after)),
        source: this.source,
        clear: [...this.cleared],
      },
    });
  }
}

export const editor = new ProfileEditor();
