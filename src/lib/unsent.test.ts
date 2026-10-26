import { describe, expect, it } from "vitest";
import { ago, clock } from "./unsent";

const MINUTE = 60_000;

describe("ago", () => {
  const now = Date.UTC(2026, 9, 26, 12);

  // The exact wording is the platform's (ICU), so only its gist is checked.
  it("says how long ago, in the interface language", () => {
    expect(ago(now - 20_000, now, "en")).toBe("now");
    expect(ago(now - 5 * MINUTE, now, "en")).toMatch(/^5 min\.? ago$/);
    expect(ago(now - 125 * MINUTE, now, "en")).toMatch(/^2 hr\.? ago$/);
    expect(ago(now - 3 * 24 * 60 * MINUTE, now, "en")).toBe("3 days ago");
    expect(ago(now - 125 * MINUTE, now, "fr")).toMatch(/^il y a 2\sh$/);
    expect(ago(now - 26 * 60 * MINUTE, now, "fr")).toBe("hier");
  });

  it("never says a time to come", () => {
    expect(ago(now + 5 * MINUTE, now, "en")).toBe("now");
  });
});

describe("clock", () => {
  it("reads like a clock", () => {
    expect(clock(42_000)).toBe("0:42");
    expect(clock(185_400)).toBe("3:05");
    expect(clock(3_723_000)).toBe("1:02:03");
    // Half a second of speech is still something.
    expect(clock(300)).toBe("0:01");
  });
});
