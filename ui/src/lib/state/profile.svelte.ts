// The profile editor (Profil view): the form as it was handed out (`before`), the form as
// the user has it (`after`) and where it came from. Saving sends both; the backend writes
// only what differs and keeps every other key of the file. A draft (a new profile, a chosen
// file, Claude's answer) is unsaved until it is saved; the stored profile only once
// something differs. Leaving the view with unsaved changes asks first (ProfileView holds
// the guard and the dialog).

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
} from '../ipc/types';

/** Where the form in the editor came from. */
export type DraftOrigin = 'stored' | 'new' | 'file' | 'answer';

/** At most this many competences are Schwerpunkte (the backend refuses more). */
export const MAX_FOCUS = 5;

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
      available: { kind: 'unset' },
      remoteOutside: false,
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

/** Form fields that can hold a value from the file the app could not read. */
export type UnreadableField = 'minSalary' | 'places' | 'remoteMin' | 'targetYears' | 'available';

/** The profile keys of those values (an external contract, German and English). */
const UNREADABLE_KEYS: Record<string, UnreadableField> = {
  min_jahresgehalt: 'minSalary',
  min_annual_salary: 'minSalary',
  min_salary: 'minSalary',
  festanstellung_orte: 'places',
  permanent_locations: 'places',
  permanent_places: 'places',
  festanstellung_remote_min: 'remoteMin',
  permanent_remote_min: 'remoteMin',
  zielprofil_min_jahre: 'targetYears',
  target_min_years: 'targetYears',
};

/** The values the engine could not read (its warnings), by form field, as the file had
 *  them (a JSON text loses its quotes). */
export function unreadableValues(
  warnings: readonly Notice[],
): Partial<Record<UnreadableField, string>> {
  const out: Partial<Record<UnreadableField, string>> = {};
  const text = (value: unknown): string =>
    typeof value === 'string' ? value.replace(/^"(.*)"$/, '$1') : String(value ?? '');
  for (const warning of warnings) {
    if (warning.code === 'availabilityNotUnderstood') {
      out.available = text(warning.params.value);
    } else if (warning.code === 'criterionNotUnderstood') {
      const field = UNREADABLE_KEYS[text(warning.params.key)];
      if (field) out[field] = text(warning.params.value);
    }
  }
  return out;
}

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

const pad = (value: number): string => String(value).padStart(2, '0');

/** `2026-11-01` -> `01.11.2026`, in English `01/11/2026` (as the field shows a day). */
export function shownDate(iso: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  const mark = language.current === 'de' ? '.' : '/';
  return match ? `${match[3]}${mark}${match[2]}${mark}${match[1]}` : iso;
}

/**
 * A typed day (`1.11.2026`, `01.11.26`, `01/11/2026`, `2026-11-01`) as `YYYY-MM-DD`; `null`
 * if it is none. Day first in both languages (German and British English).
 */
export function isoDate(text: string): string | null {
  const value = text.trim();
  const german = /^(\d{1,2})[./](\d{1,2})[./](\d{2}|\d{4})$/.exec(value);
  const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(value);
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

/** The day field's text for a form. */
const dateTextOf = (form: ProfileForm): string =>
  form.criteria.available.kind === 'from' ? shownDate(form.criteria.available.date) : '';

class ProfileEditor {
  /** `null`: nothing in the editor (no profile yet, or the view has not opened one). */
  origin = $state<DraftOrigin | null>(null);
  before = $state.raw<ProfileForm>(emptyForm());
  after = $state<ProfileForm>(emptyForm());
  source = $state<string | null>(null);
  /** How much the engine understands of a draft (the stored profile has its own). */
  quality = $state<ProfileQuality | null>(null);
  /** The steps to create a profile from a CV with Claude are open. */
  pasting = $state(false);
  /** The day of "Verfügbar ab" as typed (the form holds it as `YYYY-MM-DD`). */
  dateText = $state('');

  get dirty(): boolean {
    if (this.origin === null) return false;
    if (this.origin === 'file' || this.origin === 'answer') return true;
    return !sameForm(this.before, this.after);
  }

  /** The stored profile (again, e.g. after a save). */
  edit(form: ProfileForm): void {
    this.origin = 'stored';
    this.before = copy(form);
    this.after = copy(form);
    this.dateText = dateTextOf(form);
    this.source = null;
    this.quality = null;
    this.pasting = false;
  }

  /** An empty form for a new profile, with one empty competence and language row, so the
   *  table and the star show at once. */
  create(): void {
    this.origin = 'new';
    this.before = emptyForm();
    this.after = {
      ...emptyForm(),
      competences: [{ name: '', years: null, aliases: [], origin: null }],
      languages: [{ language: '', level: null, origin: null }],
    };
    this.dateText = '';
    this.source = NEW_SOURCE;
    this.quality = null;
    this.pasting = false;
  }

  /** A chosen file or Claude's answer, to review before it is saved. */
  take(draft: ProfileDraft, origin: 'file' | 'answer'): void {
    this.origin = origin;
    this.before = copy(draft.form);
    this.after = copy(draft.form);
    this.dateText = dateTextOf(draft.form);
    this.source = draft.source;
    this.quality = draft.quality;
    this.pasting = false;
  }

  /** Nothing in the editor (the empty state shows). */
  close(): void {
    this.origin = null;
    this.before = emptyForm();
    this.after = emptyForm();
    this.dateText = '';
    this.source = null;
    this.quality = null;
    this.pasting = false;
  }

  /** "Ab Datum" is chosen but the day does not read. */
  get dateInvalid(): boolean {
    return this.after.criteria.available.kind === 'from' && isoDate(this.dateText) === null;
  }

  /** Drops the changes: the stored profile as saved, or no draft at all. */
  discard(stored: ProfileForm | null): void {
    if (stored !== null) this.edit(stored);
    else this.close();
  }

  /** Writes the form; the caller reloads the app state (and with it the stored form). */
  save(): Promise<ProfileInfo> {
    return invoke('save_profile', {
      save: { before: this.before, after: normalized(copy(this.after)), source: this.source },
    });
  }
}

export const editor = new ProfileEditor();
