import { describe, expect, it } from "vitest";
import { translate } from "../i18n";
import type { CloudAccount } from "./api";
import { accountStatus, codeHalves, daysUntil, monthUsage, needsPlan, nextUtcDay, nextUtcMonth } from "./cloud";

const account = (patch: Partial<CloudAccount> = {}): CloudAccount => ({
  email: "name@example.com",
  status: "active",
  plan: "monthly",
  trialEndsAt: null,
  renewsAt: null,
  cancelAtPeriodEnd: false,
  usage: { daySeconds: 600, monthSeconds: 43_200, dayLimitSeconds: 14_400, monthLimitSeconds: 144_000 },
  manageUrl: "https://cloud.parler.synapsr.io/account",
  ...patch,
});

const now = new Date(2026, 9, 24, 15, 0).getTime();
const en = (key: string, params?: Record<string, string | number>) => translate("en", key, params);
const fr = (key: string, params?: Record<string, string | number>) => translate("fr", key, params);

describe("accountStatus", () => {
  it("counts the trial's days left by calendar day", () => {
    expect(daysUntil(new Date(2026, 9, 27, 9, 0).toISOString(), now)).toBe(3);
    expect(daysUntil(new Date(2026, 9, 24, 23, 0).toISOString(), now)).toBe(0);
    expect(daysUntil(new Date(2026, 9, 20).toISOString(), now)).toBe(0);
    const trial = account({ status: "trialing", trialEndsAt: new Date(2026, 9, 25, 9).toISOString() });
    expect(accountStatus(trial, en, "en", now).text).toBe("Free trial, ends tomorrow");
    expect(accountStatus(trial, fr, "fr", now).text).toBe("Essai gratuit, se termine demain");
  });

  it("says when the plan renews or ends", () => {
    const renews = account({ renewsAt: new Date(2026, 10, 24).toISOString() });
    expect(accountStatus(renews, en, "en", now)).toEqual({ text: "Monthly plan, renews on November 24", tone: "ok" });
    const cancels = account({ plan: "yearly", renewsAt: new Date(2027, 9, 24).toISOString(), cancelAtPeriodEnd: true });
    expect(accountStatus(cancels, fr, "fr", now)).toEqual({
      text: "Formule annuelle, prend fin le 24 octobre 2027",
      tone: "warning",
    });
  });

  it("asks for a plan when there is none", () => {
    expect(accountStatus(account({ status: "none" }), en, "en", now).tone).toBe("inactive");
    expect(needsPlan(account({ status: "canceled" }))).toBe(true);
    expect(needsPlan(account({ status: "past_due" }))).toBe(false);
  });
});

describe("monthUsage", () => {
  it("shows hours against the fair use limit", () => {
    // Intl separates numbers and units with a no-break space.
    const plain = (text: string) => text.replace(/\s/g, " ");
    const usage = monthUsage(account(), "fr");
    expect([plain(usage.used), plain(usage.limit), plain(usage.day)]).toEqual(["12 h", "40 h", "4 h"]);
    expect(usage.share).toBe(0.3);
  });
});

describe("fair use resets", () => {
  it("starts again at the next UTC midnight and month", () => {
    const late = new Date(Date.UTC(2026, 11, 31, 23, 30));
    expect(nextUtcDay(late).toISOString()).toBe("2027-01-01T00:00:00.000Z");
    expect(nextUtcMonth(late).toISOString()).toBe("2027-01-01T00:00:00.000Z");
  });
});

describe("codeHalves", () => {
  it("splits the code for the keycaps", () => {
    expect(codeHalves("ABCD-EFGH")).toEqual(["ABCD", "EFGH"]);
  });
});
