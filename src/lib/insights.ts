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

/** "45 min", "1 h 05", "3 h". */
export function formatDuration(minutes: number): string {
  const total = Math.round(minutes);
  if (total < 60) return `${total} min`;
  const hours = Math.floor(total / 60);
  const rest = total % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${String(rest).padStart(2, "0")}`;
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
