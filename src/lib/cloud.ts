// Parler Cloud's account, in words the interface can show.
import type { Params } from "../i18n";
import type { CloudAccount } from "./api";
import { formatDuration } from "./insights";

type T = (key: string, params?: Params) => string;

/** Midnight of the day `time` falls on, local time. */
function startOfDay(time: number): number {
  const date = new Date(time);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/** Calendar days from today to `iso`; 0 once that day has come. */
export function daysUntil(iso: string, now = Date.now()): number {
  return Math.max(0, Math.round((startOfDay(Date.parse(iso)) - startOfDay(now)) / 86_400_000));
}

/** "24 November", with the year when it isn't this one. */
export function formatDay(date: Date, locale: string, now = new Date()): string {
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "long",
    year: date.getFullYear() === now.getFullYear() ? undefined : "numeric",
  }).format(date);
}

export type AccountTone = "ok" | "warning" | "inactive";

/** Where the plan stands: "Free trial, ends in 2 days", "Monthly plan, renews on 24 November"… */
export function accountStatus(
  account: CloudAccount,
  t: T,
  locale: string,
  now = Date.now(),
): { text: string; tone: AccountTone } {
  const planKey =
    account.plan === "monthly" || account.plan === "yearly" ? account.plan : "other";
  const plan = t(`cloud.plan.${planKey}`);
  switch (account.status) {
    case "trialing": {
      if (!account.trialEndsAt) return { text: t("cloud.status.trialOpen"), tone: "ok" };
      const days = daysUntil(account.trialEndsAt, now);
      const when = new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(days, "day");
      return { text: t("cloud.status.trial", { when }), tone: "ok" };
    }
    case "active": {
      if (!account.renewsAt) return { text: plan, tone: "ok" };
      const date = formatDay(new Date(account.renewsAt), locale, new Date(now));
      const key = account.cancelAtPeriodEnd ? "cloud.status.cancels" : "cloud.status.renews";
      return { text: t(key, { plan, date }), tone: account.cancelAtPeriodEnd ? "warning" : "ok" };
    }
    case "past_due":
      return { text: t("cloud.status.pastDue"), tone: "warning" };
    case "none":
      return { text: t("cloud.status.none"), tone: "inactive" };
    case "canceled":
      return { text: t("cloud.status.canceled"), tone: "inactive" };
    default:
      return { text: t("cloud.status.unknown"), tone: "warning" };
  }
}

/** The plan doesn't allow dictating: offer to choose one. */
export const needsPlan = (account: CloudAccount) =>
  account.status === "none" || account.status === "canceled";

/** This month's dictation time against the fair use limit. */
export function monthUsage(account: CloudAccount, locale: string) {
  const { monthSeconds, monthLimitSeconds, dayLimitSeconds } = account.usage;
  return {
    used: formatDuration(monthSeconds / 60, locale),
    limit: formatDuration(monthLimitSeconds / 60, locale),
    day: formatDuration(dayLimitSeconds / 60, locale),
    share: monthLimitSeconds > 0 ? Math.min(1, monthSeconds / monthLimitSeconds) : 0,
  };
}

/** Fair use counts by UTC day and month: when each starts again. */
export function nextUtcDay(now = new Date()): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1));
}

export function nextUtcMonth(now = new Date()): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + 1, 1));
}

export function formatTime(date: Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(date);
}

/** "ABCD-EFGH" as the two halves drawn as keys. */
export const codeHalves = (code: string) => code.split("-").filter(Boolean);
