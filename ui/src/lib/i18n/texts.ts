// Turns codes with params (errors, notices, reasons, criteria, health) into catalog text.
// The screens never build sentences themselves.
//
// Engine codes travel as plain strings (`Reason.code`, `Notice.code`). The catalog tables
// in de.ts are the one place that knows them: a new code needs one entry there. Unknown
// codes from a newer core fall back to the ad's words or are left out, never to a raw code.

import { IpcError } from '../ipc/api';
import type { JobView, KeyFacts, Notice, PortalHealth, Reason } from '../ipc/types';
import { formatDate } from './format';
import {
  de,
  textOf,
  type CriterionKey,
  type CriterionState,
  type MatchNote,
  type ProfileWarning,
  type ReasonCode,
} from './de';

const has = <T extends object>(table: T, key: string): key is Extract<keyof T, string> =>
  Object.prototype.hasOwnProperty.call(table, key);

/** The text of any failure (IpcError from api.ts, or something unexpected). */
export function errorText(error: unknown): string {
  if (error instanceof IpcError) return de.error.text(error.kind, error.params);
  return de.error.text('unknown', {});
}

/** Criterion names differ between a note (`dayRate`) and the key (`minDayRate`). */
const ALIASES: Record<string, CriterionKey> = {
  dayRate: 'minDayRate',
  country: 'countries',
  anue: 'noAnue',
  salary: 'minSalary',
  tooJunior: 'targetYears',
};

export function criterionKey(value: unknown): CriterionKey | null {
  if (typeof value !== 'string') return null;
  if (has(de.reader.criterion, value)) return value;
  return ALIASES[value] ?? null;
}

/** The words of a reason: the ad's quote for requirements, a catalog sentence otherwise. */
export function reasonText(reason: Reason): string {
  const code = reason.code;
  if (!has(de.reason.code, code) || code === 'requirement' || code === 'term') {
    return reason.label;
  }
  return textOf(de.reason.code[code as ReasonCode], reason.params) || reason.label;
}

/** The line under a reason: the profile's side of its evidence (null without one). */
export function reasonEvidence(reason: Reason): string | null {
  if (!reason.evidence?.profile) return null;
  return de.reason.evidenceLine(reason.evidence.profile, reason.kind === 'partial');
}

/** Tooltip of a reason: quote and profile evidence, or that the profile lacks it. */
export function reasonHint(reason: Reason): string | null {
  if (reason.evidence) {
    return de.reason.evidence(
      reason.evidence.quote || reason.label,
      reason.evidence.profile,
      reason.kind === 'partial',
    );
  }
  if (reason.kind === 'open' && reason.label) return de.reason.missing(reason.label);
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
    return key ? de.reader.criterion[key].exclusion : de.reader.note.hardCriterion;
  }
  if (has(de.reader.note, note.code)) {
    return textOf(de.reader.note[note.code as MatchNote], note.params);
  }
  if (has(de.reason.code, note.code) && note.code !== 'requirement' && note.code !== 'term') {
    const text = textOf(de.reason.code[note.code as ReasonCode], note.params);
    if (text) return text;
  }
  const key = criterionKey(note.code);
  return key ? de.reader.criterion[key].exclusion : null;
}

/** The reason line of a list row: the exclusion note, else the best met requirement. */
export function rowReason(job: JobView): { kind: 'met' | 'violation'; text: string } | null {
  const match = job.match;
  if (match === null) return null;
  if (match.status === 'excluded') {
    return { kind: 'violation', text: noteText(match.note) ?? de.score.excluded };
  }
  const top = match.top[0];
  return top ? { kind: 'met', text: top } : null;
}

/** The start of an ad in words (`now`, `vague` or an ISO date); `vague` only when asked. */
function startWords(start: unknown, vague: boolean): string | null {
  if (start === 'now') return de.facts.now;
  if (start === 'vague') return vague ? de.facts.vague : null;
  if (typeof start === 'string' && start !== '') return de.facts.from(formatDate(start));
  return null;
}

function rateWords(
  rate: unknown,
  hourly: unknown,
  currency: unknown,
  unit: boolean,
): string | null {
  if (typeof rate !== 'number') return null;
  return de.facts.rate(
    rate,
    hourly === true,
    typeof currency === 'string' && currency !== '' ? currency : null,
    unit,
  );
}

/**
 * The key facts of an ad for its list row, in this order: start, duration, remote share,
 * rate ("ab sofort", "6 Monate", "60 % remote", "1.100 €"). What the ad does not say is left out.
 */
export function factWords(facts: KeyFacts | null | undefined): string[] {
  if (!facts) return [];
  const out: string[] = [];
  const start = startWords(facts.start, false);
  if (start) out.push(start);
  if (facts.months) out.push(de.facts.months(facts.months));
  const from = facts.remoteFrom ?? facts.remoteTo;
  const to = facts.remoteTo ?? facts.remoteFrom;
  if (from !== null && to !== null) out.push(de.facts.remote(from, to));
  const rate =
    rateWords(facts.rate, facts.hourly, facts.currency, false) ??
    (facts.rateOpen ? de.facts.rateOpen : null);
  if (rate) out.push(rate);
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
        (p.rateOpen === true ? de.facts.rateOpen : null)
      );
    case 'countries':
    case 'permanentRegion':
      if (p.remote === true) return de.facts.fullRemote;
      return typeof p.location === 'string' && p.location !== '' ? p.location : null;
    case 'noAnue':
      return typeof p.contract === 'string' && has(de.facts.contract, p.contract)
        ? de.facts.contract[p.contract]
        : null;
    case 'availability':
      return startWords(p.start, true);
    case 'minSalary':
      return typeof p.salary === 'number' ? de.facts.salary(p.salary) : null;
    case 'targetYears':
      return typeof p.years === 'number' ? de.facts.years(p.years) : null;
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

/** A hard criterion of the profile as a row: its name and its value (null = not set). */
export function profileCriterion(notice: Notice): { field: string; value: string | null } | null {
  const key = criterionKey(notice.code);
  if (key === null) return null;
  const text = de.reader.criterion[key];
  return {
    field: text.field,
    value: notice.params.set === false ? null : text.value(notice.params),
  };
}

export function warningText(notice: Notice): string | null {
  return has(de.profile.warning, notice.code)
    ? textOf(de.profile.warning[notice.code as ProfileWarning], notice.params)
    : null;
}

/** One sentence for a portal's health (`ok` has none). */
export function healthSentence(health: PortalHealth): string | null {
  switch (health.kind) {
    case 'ok':
      return null;
    case 'paused':
      return de.run.pausedWhy(health.reason, health.until);
    case 'quotaReached':
      return de.run.quota(health.until);
    case 'layoutSuspect':
      // Empty alert mails point at the mail format; otherwise the pages looked odd.
      return health.emptyMails > 0
        ? de.health.layoutText(health.emptyMails)
        : de.health.layoutPages;
    case 'loginRequired':
      return de.health.loginText;
  }
}

/**
 * A portal problem as the settings say it: one sentence that says whether anything is to
 * be done (`act`), or that the app carries on by itself.
 */
export function healthAdvice(health: PortalHealth): { text: string; act: boolean } | null {
  switch (health.kind) {
    case 'ok':
      return null;
    case 'paused':
      return { text: de.health.advice.paused(health.reason, health.until), act: false };
    case 'quotaReached':
      return { text: de.health.advice.quota(health.until), act: false };
    case 'layoutSuspect':
      return health.emptyMails > 0
        ? { text: de.health.advice.emptyMails(health.emptyMails), act: true }
        : { text: de.health.advice.pages, act: false };
    case 'loginRequired':
      return { text: de.health.advice.login, act: true };
  }
}

/**
 * The old shape of `healthSentence`, kept only until DayOverview and RunCard switch to it.
 * The short labels it once returned never showed (every problem has its sentence), so
 * `label` is that sentence too.
 */
export function healthText(health: PortalHealth): { label: string; text: string | null } {
  const text = healthSentence(health);
  return { label: text ?? '', text };
}
