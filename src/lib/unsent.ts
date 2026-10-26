// How an unsent dictation is described: how long ago, and how long it is.

/** "now", "5 min ago", "2 hr. ago", "3 days ago", in the interface language. */
export function ago(at: number, now: number, locale: string): string {
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  const minutes = Math.floor(Math.max(0, now - at) / 60_000);
  if (minutes < 1) return format.format(0, "second");
  if (minutes < 60) return format.format(-minutes, "minute");
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return format.format(-hours, "hour");
  return format.format(-Math.floor(hours / 24), "day");
}

/** A length as a clock reads it: "0:42", "3:05", "1:02:03". */
export function clock(ms: number): string {
  const total = Math.max(1, Math.round(ms / 1000));
  const seconds = String(total % 60).padStart(2, "0");
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}`
    : `${minutes}:${seconds}`;
}
