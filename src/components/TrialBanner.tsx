import { X } from "lucide-react";
import { useState } from "react";
import { useT } from "../i18n/context";
import { daysUntil } from "../lib/cloud";
import { useParler } from "../state";
import { Banner, IconButton } from "../ui";
import { PlanButton } from "./Cloud";

/** A trial ending today or tomorrow gets a word on Home. */
const SOON_DAYS = 1;

const dismissedKey = (trialEndsAt: string) => `parler.trialBanner.${trialEndsAt}`;

function wasDismissed(trialEndsAt: string): boolean {
  try {
    return localStorage.getItem(dismissedKey(trialEndsAt)) === "1";
  } catch {
    return false;
  }
}

/**
 * Parler Cloud's trial ends soon: say what happens next before it does. The
 * plan takes over with nothing to do, or, when the trial was cancelled,
 * dictation stops. Dismissed once per trial, so it never nags.
 */
export function TrialBanner() {
  const { t, locale } = useT();
  const { cloud } = useParler();
  const account = cloud.account;
  const trialEndsAt = account?.status === "trialing" ? account.trialEndsAt : null;
  const [dismissed, setDismissed] = useState(() => (trialEndsAt ? wasDismissed(trialEndsAt) : false));

  if (!account || !trialEndsAt || dismissed) return null;
  const days = daysUntil(trialEndsAt);
  if (days > SOON_DAYS) return null;

  // "today", "tomorrow".
  const when = new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(days, "day");
  const dismiss = () => {
    setDismissed(true);
    try {
      localStorage.setItem(dismissedKey(trialEndsAt), "1");
    } catch {
      // Private mode: it comes back next time, which is fine.
    }
  };

  return (
    <Banner
      tone="info"
      actions={
        <>
          <PlanButton account={account} quiet={!account.cancelAtPeriodEnd} />
          <IconButton label={t("common.hide")} onClick={dismiss} className="ml-auto">
            <X className="size-4" />
          </IconButton>
        </>
      }
    >
      {t(account.cancelAtPeriodEnd ? "cloud.trialEnding.stops" : "cloud.trialEnding.continues", { when })}
    </Banner>
  );
}
