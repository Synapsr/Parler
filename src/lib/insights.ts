import type { HistoryEntry, Stats } from "./api";

/** People type about 40 words a minute; speech is the time actually spoken. */
const TYPING_WPM = 40;

export function minutesSaved(stats: Stats): number {
  const typing = stats.words / TYPING_WPM;
  const speaking = stats.speechMs / 60_000;
  return Math.max(0, typing - speaking);
}

export function wordsPerMinute(stats: Stats): number {
  return stats.speechMs > 0 ? Math.round(stats.words / (stats.speechMs / 60_000)) : 0;
}

/** "45 min", "1 h 5 min", "12 h", with the units of the interface language. */
export function formatDuration(minutes: number, locale: string): string {
  const unit = (unit: "hour" | "minute", value: number) =>
    new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "short" }).format(value);
  const total = Math.round(minutes);
  if (total < 60) return unit("minute", total);
  // Past ten hours, the minutes are noise.
  if (total >= 600) return unit("hour", Math.round(total / 60));
  const hours = Math.floor(total / 60);
  const rest = total % 60;
  if (rest === 0) return unit("hour", hours);
  // Chinese and Japanese write it without a space: 3小时9分钟.
  const gap = locale === "ja" || locale === "zh" ? "" : " ";
  return `${unit("hour", hours)}${gap}${unit("minute", rest)}`;
}

export interface Day {
  /** Midnight of that day, local time. */
  start: number;
  entries: HistoryEntry[];
}

/** Entries grouped by local day, newest day first. */
export function groupByDay(entries: HistoryEntry[]): Day[] {
  const days: Day[] = [];
  for (const entry of entries) {
    const date = new Date(entry.id);
    const start = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
    const last = days[days.length - 1];
    if (last?.start === start) last.entries.push(entry);
    else days.push({ start, entries: [entry] });
  }
  return days;
}

/** "Today", "Yesterday", or the date, in the interface language. */
export function dayLabel(start: number, now: number, locale: string): string {
  const today = new Date(now);
  const midnight = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const days = Math.round((midnight - start) / 86_400_000);
  if (days <= 1) {
    const label = new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(-days, "day");
    return label.charAt(0).toUpperCase() + label.slice(1);
  }
  return new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" })
    .format(start)
    .replace(/^./, (c) => c.toUpperCase());
}
