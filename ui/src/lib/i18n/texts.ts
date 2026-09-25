// Turns codes with params (errors, notices, reasons, criteria, health) into catalog text of
// the app's language (`t`). The screens never build sentences themselves.
//
// Engine codes travel as plain strings (`Reason.code`, `Notice.code`). The catalog tables
// in t.ts (and en.ts, the same keys) are the one place that knows them: a new code needs one
// entry there. Unknown codes from a newer core fall back to the ad's words or are left out,
// never to a raw code.

import { IpcError } from '../ipc/api';
import type { DetailState, JobView, KeyFacts, Notice, PortalHealth, Reason } from '../ipc/types';
import { formatDate } from './format';
import {
  textOf,
  type CriterionKey,
  type CriterionState,
  type MatchNote,
  type ProfileWarning,
  type ReasonCode,
} from './de';
import { t } from './t';

const has = <T extends object>(table: T, key: string): key is Extract<keyof T, string> =>
  Object.prototype.hasOwnProperty.call(table, key);

/** The text of any failure (IpcError from api.ts, or something unexpected). */
export function errorText(error: unknown): string {
  if (error instanceof IpcError) return t.error.text(error.kind, error.params);
  return t.error.text('unknown', {});
}

/** Criterion names differ between a note (`dayRate`) and the key (`minDayRate`). */
const ALIASES: Record<string, CriterionKey> = {
  dayRate: 'minDayRate',
  country: 'countries',
  anue: 'noAnue',
  permanent: 'noPermanent',
  salary: 'minSalary',
  tooJunior: 'targetYears',
};

export function criterionKey(value: unknown): CriterionKey | null {
  if (typeof value !== 'string') return null;
  if (has(t.reader.criterion, value)) return value;
  return ALIASES[value] ?? null;
}

/** The words of a reason: the ad's quote for requirements, a catalog sentence otherwise. */
export function reasonText(reason: Reason): string {
  const code = reason.code;
  if (!has(t.reason.code, code) || code === 'requirement' || code === 'term') {
    return reason.label;
  }
  return textOf(t.reason.code[code as ReasonCode], reason.params) || reason.label;
}

/** Wishes of the profile: their sentence names the wish already. */
const WISH_CODES: readonly string[] = ['dayRateWish', 'remoteWish', 'regionWish', 'industryWish'];
const same = (a: string, b: string): boolean =>
  a.trim().toLocaleLowerCase() === b.trim().toLocaleLowerCase();

/** The line under a reason: the profile's side of its evidence; null without one, for a wish
 *  and when the profile says the very words of the reason. */
export function reasonEvidence(reason: Reason): string | null {
  const profile = reason.evidence?.profile;
  if (!profile || WISH_CODES.includes(reason.code)) return null;
  if (same(profile, reason.label) || same(profile, reason.evidence?.quote ?? '')) return null;
  return t.reason.evidenceLine(profile, reason.kind === 'partial');
}

/** Tooltip of a reason: quote and profile evidence, or that the profile lacks it. */
export function reasonHint(reason: Reason): string | null {
  if (reason.evidence) {
    return t.reason.evidence(
      reason.evidence.quote || reason.label,
      reason.evidence.profile,
      reason.kind === 'partial',
    );
  }
  if (reason.kind === 'open' && reason.label) return t.reason.missing(reason.label);
  return null;
}

/**
 * Why a job is excluded or not scored (the note of its match), if there is one. The engine
 * names the first violation by its reason code (`dayRate`, `country`, `anue` ...) with that
 * reason's params, so after the note table the reason catalog speaks, then the criterion.
 */
export function noteText(note: Notice | null): string | null {
  if (note === null) return null;
  if (note.code === 'hardCriterion') {
    const key = criterionKey(note.params.criterion);
    return key ? t.reader.criterion[key].exclusion : t.reader.note.hardCriterion;
  }
  if (has(t.reader.note, note.code)) {
    return textOf(t.reader.note[note.code as MatchNote], note.params);
  }
  if (has(t.reason.code, note.code) && note.code !== 'requirement' && note.code !== 'term') {
    const text = textOf(t.reason.code[note.code as ReasonCode], note.params);
    if (text) return text;
  }
  const key = criterionKey(note.code);
  return key ? t.reader.criterion[key].exclusion : null;
}

/** The criterion an exclusion note names (`hardCriterion` with its key, or the reason code). */
function noteCriterion(note: Notice | null): CriterionKey | null {
  if (note === null) return null;
  return note.code === 'hardCriterion'
    ? criterionKey(note.params.criterion)
    : criterionKey(note.code);
}

/** Whether a detail state warns (the ad could not be read, or is gone) or is a quiet fact
 *  (it follows, it is a teaser, it comes on request): one tone for the row's badge, the
 *  reader's note and the run card. */
export const DETAIL_WARNS: Record<Exclude<DetailState['kind'], 'ok'>, boolean> = {
  failed: true,
  unfetchable: true,
  gone: true,
  pending: false,
  teaser: false,
  onRequest: false,
};

/** The reason line of a list row: why it is excluded in short words ("Tagessatz zu
 *  niedrig"), else the best met requirement. */
export function rowReason(job: JobView): { kind: 'met' | 'violation'; text: string } | null {
  const match = job.match;
  if (match === null) return null;
  if (match.status === 'excluded') return { kind: 'violation', text: exclusionWords(match.note) };
  const top = match.top[0];
  return top ? { kind: 'met', text: top } : null;
}

/**
 * Why a job is excluded in the short words of a row, never a sentence (the reader has
 * those): the criterion, a mandatory degree or licence the profile lacks, else only
 * "Ausgeschlossen" (a code of a newer core).
 */
function exclusionWords(note: Notice | null): string {
  const key = noteCriterion(note);
  if (key) return t.reader.criterion[key].short;
  if (note?.code === 'formalOpen') {
    return t.list.formalMissing[note.params.class === 'licence' ? 'licence' : 'degree'];
  }
  return t.score.excluded;
}

/** The start of an ad in words (`now`, `vague` or an ISO date); `vague` only when asked. */
function startWords(start: unknown, vague: boolean): string | null {
  if (start === 'now') return t.facts.now;
  if (start === 'vague') return vague ? t.facts.vague : null;
  if (typeof start === 'string' && start !== '') return t.facts.from(formatDate(start));
  return null;
}

function rateWords(
  rate: unknown,
  hourly: unknown,
  currency: unknown,
  unit: boolean,
): string | null {
  if (typeof rate !== 'number') return null;
  return t.facts.rate(
    rate,
    hourly === true,
    typeof currency === 'string' && currency !== '' ? currency : null,
    unit,
  );
}

/**
 * The key facts of an ad for its list row, in this order: start, duration, remote share,
 * rate ("ab sofort", "6 Monate", "60 % remote", "1.100 €/Tag"). What the ad does not say is
 * left out.
 */
export function factWords(facts: KeyFacts | null | undefined): string[] {
  if (!facts) return [];
  const out: string[] = [];
  const start = startWords(facts.start, false);
  if (start) out.push(start);
  if (facts.months) out.push(t.facts.months(facts.months));
  const from = facts.remoteFrom ?? facts.remoteTo;
  const to = facts.remoteTo ?? facts.remoteFrom;
  if (from !== null && to !== null) out.push(t.facts.remote(from, to));
  const rate =
    rateWords(facts.rate, facts.hourly, facts.currency, true) ??
    (facts.rateOpen ? t.facts.rateOpen : null);
  if (rate) out.push(rate);
  return out;
}

/** The duration and remote share of an ad (the reader's facts line, in place of the work
 *  mode when the ad says more). */
export function workWords(facts: KeyFacts | null | undefined): string[] {
  if (!facts) return [];
  const out: string[] = [];
  if (facts.months) out.push(t.facts.months(facts.months));
  const from = facts.remoteFrom ?? facts.remoteTo;
  const to = facts.remoteTo ?? facts.remoteFrom;
  if (from !== null && to !== null) out.push(t.facts.remote(from, to));
  return out;
}

/**
 * What the ad says about a hard criterion, in its own value ("1.100 €/Tag", "ab sofort",
 * "Hamburg", "Interim"); `null` when it says nothing (the chip then names the criterion).
 */
export function criterionValue(reason: Reason): string | null {
  const p = reason.params;
  switch (criterionKey(reason.code)) {
    case 'minDayRate':
      return (
        rateWords(p.rate, p.hourly, p.currency, true) ??
        (p.rateOpen === true ? t.facts.rateOpen : null)
      );
    case 'countries':
    case 'permanentRegion':
      if (p.remote === true) return t.facts.fullRemote;
      return typeof p.location === 'string' && p.location !== '' ? p.location : null;
    case 'noAnue':
    case 'noPermanent':
      return typeof p.contract === 'string' && has(t.facts.contract, p.contract)
        ? t.facts.contract[p.contract]
        : null;
    case 'availability':
      return startWords(p.start, true);
    case 'minSalary':
      return typeof p.salary === 'number' ? t.facts.salary(p.salary) : null;
    case 'targetYears':
      return typeof p.years === 'number' ? t.facts.years(p.years) : null;
    default:
      return null;
  }
}

/** A criterion of the reader strip (kind met = fulfilled with the ad as evidence, violation, check = unclear, open = the ad does not mention it). */
export function criterionState(reason: Reason): CriterionState {
  switch (reason.kind) {
    case 'met':
    case 'partial':
      return 'met';
    case 'violation':
      return 'violated';
    case 'check':
      return 'unknown';
    default:
      return 'unset';
  }
}

export function warningText(notice: Notice): string | null {
  return has(t.profile.warning, notice.code)
    ? textOf(t.profile.warning[notice.code as ProfileWarning], notice.params)
    : null;
}

/**
 * A portal problem in one sentence that says what she has to do, or that the app carries on
 * by itself (which of the two is `PortalState.actionNeeded`); the same words in the run
 * card, the day overview and the settings. `ok` has none.
 */
export function healthAdvice(health: PortalHealth): string | null {
  switch (health.kind) {
    case 'ok':
      return null;
    case 'paused':
      return t.health.advice.paused(health.reason, health.until);
    case 'quotaReached':
      return t.health.advice.quota(health.until);
    case 'layoutSuspect':
      return health.emptyMails > 0
        ? t.health.advice.emptyMails(health.emptyMails)
        : t.health.advice.pages;
    case 'loginRequired':
      return t.health.advice.login;
  }
}
