import { describe, expect, it } from "vitest";
import { dayLabel, formatDuration, groupByDay, minutesSaved, wordsPerMinute } from "./insights";

describe("stats", () => {
  it("counts the typing time a dictation spared", () => {
    // 400 words take 10 minutes to type; they took 3 minutes to say.
    expect(minutesSaved({ dictations: 5, words: 400, speechMs: 180_000 })).toBe(7);
    expect(minutesSaved({ dictations: 0, words: 0, speechMs: 0 })).toBe(0);
  });

  it("reports speaking speed", () => {
    expect(wordsPerMinute({ dictations: 1, words: 150, speechMs: 60_000 })).toBe(150);
    expect(wordsPerMinute({ dictations: 0, words: 0, speechMs: 0 })).toBe(0);
  });

  it("formats durations", () => {
    expect(formatDuration(7.4)).toBe("7 min");
    expect(formatDuration(65)).toBe("1 h 05");
    expect(formatDuration(180)).toBe("3 h");
  });
});

describe("history days", () => {
  const at = (y: number, m: number, d: number, h = 12) => new Date(y, m, d, h).getTime();
  const entry = (id: number) => ({ id, text: "", durationMs: 0, provider: "Groq" });

  it("groups newest-first entries by day", () => {
    const days = groupByDay([entry(at(2026, 9, 23, 18)), entry(at(2026, 9, 23, 9)), entry(at(2026, 9, 21))]);
    expect(days.map((d) => d.entries.length)).toEqual([2, 1]);
  });

  it("names today and yesterday", () => {
    const now = at(2026, 9, 23, 20);
    expect(dayLabel(at(2026, 9, 23, 0), now, "fr")).toBe("Aujourd’hui");
    expect(dayLabel(at(2026, 9, 22, 0), now, "en")).toBe("Yesterday");
    expect(dayLabel(at(2026, 9, 20, 0), now, "fr")).toBe("Mardi 20 octobre");
  });
});
