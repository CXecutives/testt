// Turns codes with params (errors, notices, reasons, criteria, health) into catalog text of
// the app's language (`t`). The screens never build sentences themselves.
//
// Engine codes travel as plain strings (`Reason.code`, `Notice.code`). The catalog tables
// in de.ts (and en.ts, the same keys) are the one place that knows them: a new code needs one
// entry there. Unknown codes from a newer core fall back to the ad's words or are left out,
// never to a raw code.

import { IpcError } from '../ipc/api';
import type { JobView, Notice, PortalHealth, Reason } from '../ipc/types';
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

/** The reason line of a list row: the exclusion note, else the best met requirement. */
export function rowReason(job: JobView): { kind: 'met' | 'violation'; text: string } | null {
  const match = job.match;
  if (match === null) return null;
  if (match.status === 'excluded') {
    return { kind: 'violation', text: noteText(match.note) ?? t.score.excluded };
  }
  const top = match.top[0];
  return top ? { kind: 'met', text: top } : null;
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

/** One sentence for a portal's health (`ok` has none). */
export function healthSentence(health: PortalHealth): string | null {
  switch (health.kind) {
    case 'ok':
      return null;
    case 'paused':
      return t.run.pausedWhy(health.reason, health.until);
    case 'quotaReached':
      return t.run.quota(health.until);
    case 'layoutSuspect':
      // Empty alert mails point at the mail format; otherwise the pages looked odd.
      return health.emptyMails > 0 ? t.health.layoutText(health.emptyMails) : t.health.layoutPages;
    case 'loginRequired':
      return t.health.loginText;
  }
}

/**
 * A portal problem as the settings say it: one sentence that says what she has to do, or
 * that the app carries on by itself (which of the two is `PortalState.actionNeeded`).
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

/**
 * The old shape of `healthSentence`, kept only until DayOverview and RunCard switch to it.
 * The short labels it once returned never showed (every problem has its sentence), so
 * `label` is that sentence too.
 */
export function healthText(health: PortalHealth): { label: string; text: string | null } {
  const text = healthSentence(health);
  return { label: text ?? '', text };
}
