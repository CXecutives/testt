// Numbers and dates for the German UI. Render the results with tabular numbers
// (`font-variant-numeric: var(--numeric)`), so counters do not jitter.

const LOCALE = 'de-DE';
/** U+202F, the narrow no-break space between a number and its unit (`87 %`). */
export const NARROW_NBSP = ' ';

const integer = new Intl.NumberFormat(LOCALE, { maximumFractionDigits: 0 });
const relative = new Intl.RelativeTimeFormat(LOCALE, { numeric: 'auto' });
const dayMonth = new Intl.DateTimeFormat(LOCALE, { day: '2-digit', month: '2-digit' });
const dayMonthYear = new Intl.DateTimeFormat(LOCALE, {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});
const clock = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit' });

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const RELATIVE_DAYS = 7;

/** `1.234` */
export function formatNumber(value: number): string {
  return integer.format(value);
}

/** `87 %` with a narrow no-break space. */
export function formatPercent(value: number): string {
  return `${integer.format(value)}${NARROW_NBSP}%`;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/**
 * `gerade eben` · `vor 5 Minuten` · `vor 3 Stunden` · `gestern` · `vor 4 Tagen`, then
 * `12.09.` (with the year if it is not the current one).
 */
export function formatRelative(iso: string, now: Date = new Date()): string {
  const date = new Date(iso);
  const time = date.getTime();
  if (Number.isNaN(time)) return '';
  const diff = now.getTime() - time;
  const days = Math.round((startOfDay(now) - startOfDay(date)) / DAY);
  if (diff >= 0 && days === 0) {
    if (diff < MINUTE) return relative.format(0, 'second');
    if (diff < HOUR) return relative.format(-Math.floor(diff / MINUTE), 'minute');
    return relative.format(-Math.floor(diff / HOUR), 'hour');
  }
  if (days > 0 && days <= RELATIVE_DAYS) return relative.format(-days, 'day');
  return date.getFullYear() === now.getFullYear()
    ? dayMonth.format(date)
    : dayMonthYear.format(date);
}

/** `14:05` */
export function formatTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? '' : clock.format(date);
}
