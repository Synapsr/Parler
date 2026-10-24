// Parler Cloud in the interface: the code to check while signing in, and the
// account once signed in.
import { ArrowUpRight, ChevronDown, LoaderCircle, RotateCw } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { joinSentences } from "../i18n";
import { useT } from "../i18n/context";
import { api, type CloudAccount } from "../lib/api";
import { accountStatus, codeHalves, monthUsage, needsPlan, type AccountTone } from "../lib/cloud";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";
import { Banner, Button, Keycaps } from "../ui";
import { useCloudSignIn, type CloudSignIn } from "./useCloudSignIn";

const open = (url: string) => openUrl(url).catch(() => {});

/** Whether the usage is unfolded: kept while Parler runs, folded at launch. */
let usageShown = false;

/** "Check that your browser shows" ABCD – EFGH, drawn as keys. */
export function CloudCode({ code, size = "xl" }: { code: string; size?: "lg" | "xl" }) {
  const { t } = useT();
  const [first, second] = codeHalves(code);
  return (
    <div className="flex flex-col gap-3">
      <p className="text-[13.5px] text-text-2">{t("cloud.checkCode")}</p>
      <div data-selectable className="flex items-center gap-2.5" role="group" aria-label={code}>
        <Keycaps keys={[first ?? code]} size={size} />
        {second && (
          <>
            <span className="font-mono text-[18px] text-text-3" aria-hidden="true">
              –
            </span>
            <Keycaps keys={[second]} size={size} />
          </>
        )}
      </div>
    </div>
  );
}

/** The browser has the next step: say so, calmly. */
export function SignInWaiting({ signIn, size }: { signIn: CloudSignIn; size?: "lg" | "xl" }) {
  const { t } = useT();
  if (signIn.state.phase !== "waiting") return null;
  return (
    <div className="rise flex flex-col gap-5">
      <CloudCode code={signIn.state.start.userCode} size={size} />
      <p className="flex items-center gap-2 text-[13.5px] text-text-2" role="status">
        <LoaderCircle className="size-4 animate-spin text-text-3" />
        {t("cloud.waiting")}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={signIn.reopen}>
          {t("cloud.reopen")}
          <ArrowUpRight className="size-3.5" />
        </Button>
        <Button size="sm" variant="ghost" onClick={signIn.cancel}>
          {t("common.cancel")}
        </Button>
      </div>
    </div>
  );
}

/** What went wrong with a sign-in, or nothing. */
export function signInTrouble(signIn: CloudSignIn, t: (key: string) => string): string | null {
  switch (signIn.state.phase) {
    case "denied":
      return t("cloud.denied");
    case "expired":
      return t("cloud.expired");
    case "failed":
      return signIn.state.message;
    default:
      return null;
  }
}

const DOT: Record<AccountTone, string> = {
  ok: "bg-success",
  warning: "bg-warning",
  inactive: "bg-accent",
};

/** Where the plan stands, after a coloured light. */
export function AccountStatus({ account }: { account: CloudAccount }) {
  const { t, locale } = useT();
  const status = accountStatus(account, t, locale);
  return (
    <span className="flex items-start gap-2 text-[12.5px] leading-snug text-text-2">
      <span className={`mt-[5px] size-1.5 shrink-0 rounded-full ${DOT[status.tone]}`} />
      {status.text}
    </span>
  );
}

/** The button that leads to the plan: choose one, or manage it. */
export function PlanButton({
  account,
  quiet = false,
}: {
  account: Pick<CloudAccount, "status" | "manageUrl">;
  /** Next to another main action, such as "Try again". */
  quiet?: boolean;
}) {
  const { t } = useT();
  const choose = needsPlan(account as CloudAccount);
  // Without a plan, or with a card to update, it's the thing to do.
  const urgent = !quiet && (choose || account.status === "past_due");
  return (
    <Button size="sm" variant={urgent ? "primary" : "secondary"} onClick={() => open(account.manageUrl)}>
      {t(choose ? "cloud.choosePlan" : "cloud.manage")}
      <ArrowUpRight className="size-3.5" />
    </Button>
  );
}

/** Email, plan, this month's use, and the buttons to manage it. */
function AccountDetails({ account }: { account: CloudAccount }) {
  const { t, locale } = useT();
  const [leaving, setLeaving] = useState(false);
  const [showUsage, setShowUsage] = useState(usageShown);
  const toggleUsage = () => {
    usageShown = !showUsage;
    setShowUsage(usageShown);
  };
  const usage = monthUsage(account, locale);
  const signOut = async () => {
    setLeaving(true);
    try {
      await api.cloudSignOut();
    } finally {
      setLeaving(false);
    }
  };

  return (
    <div className="flex flex-col gap-3.5">
      <div className="flex items-center gap-3">
        <span
          aria-hidden="true"
          className="flex size-9 shrink-0 items-center justify-center rounded-full bg-ink font-display text-[18px] text-on-ink"
        >
          {account.email.charAt(0).toUpperCase()}
        </span>
        <div className="min-w-0 flex-1">
          <div data-selectable className="truncate text-[14px] font-medium text-text">
            {account.email}
          </div>
          <AccountStatus account={account} />
        </div>
      </div>

      {!needsPlan(account) && (
        <div>
          {/* Folded like the other secondary details. */}
          <button
            type="button"
            onClick={toggleUsage}
            aria-expanded={showUsage}
            aria-controls="cloud-usage"
            className="inline-flex items-center gap-1 text-[13px] font-medium text-text-2 hover:text-text"
          >
            <ChevronDown
              className={`size-4 transition-transform duration-200 ${showUsage ? "" : "-rotate-90"}`}
            />
            {t("cloud.usageToggle")}
          </button>
          {showUsage && (
            <div
              id="cloud-usage"
              className="rise mt-3 rounded-xl bg-surface-2 px-3.5 py-3 shadow-[inset_0_0_0_1px_var(--border)]"
            >
              <div className="flex items-baseline justify-between gap-3 text-[12.5px]">
                <span className="text-text-2">{t("cloud.usage")}</span>
                <span className="font-medium text-text tabular-nums">
                  {t("cloud.usageOf", { used: usage.used, limit: usage.limit })}
                </span>
              </div>
              <div
                className="mt-2 h-1.5 overflow-hidden rounded-full bg-text/10"
                role="meter"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(usage.share * 100)}
                aria-label={t("cloud.usage")}
              >
                <div
                  className={`h-full rounded-full transition-[width] duration-500 ${
                    usage.share >= 1 ? "bg-accent" : usage.share >= 0.85 ? "bg-warning" : "bg-text"
                  }`}
                  style={{ width: `${Math.max(usage.share * 100, usage.share > 0 ? 2 : 0)}%` }}
                />
              </div>
              <p className="mt-2 text-[12px] text-text-3">
                {t("cloud.fairUse", { day: usage.day, month: usage.limit })}
              </p>
            </div>
          )}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <PlanButton account={account} />
        <Button size="sm" variant="ghost" onClick={signOut} busy={leaving}>
          {t("cloud.signOut")}
        </Button>
      </div>
    </div>
  );
}

/** Settings: the account when signed in, the sign-in itself otherwise. */
export function CloudAccountPanel() {
  const { t, locale } = useT();
  const { cloud } = useParler();
  const signIn = useCloudSignIn();

  if (!cloud.signedIn) {
    const trouble = signInTrouble(signIn, t);
    const waiting = signIn.state.phase === "waiting";
    return (
      <div className="flex flex-col gap-3.5">
        {!waiting && (
          <p className="max-w-[56ch] text-[13px] leading-relaxed text-text-2">
            {joinSentences(locale, t("onboarding.signIn.subtitle"), t("cloud.price"))}
          </p>
        )}
        {trouble && <Banner tone={signIn.state.phase === "expired" ? "warning" : "error"}>{trouble}</Banner>}
        {waiting ? (
          <SignInWaiting signIn={signIn} size="lg" />
        ) : (
          <div>
            <Button variant="primary" onClick={signIn.start} busy={signIn.state.phase === "starting"}>
              {trouble ? t("problems.retry") : t("cloud.start")}
              <ArrowUpRight className="size-4" />
            </Button>
          </div>
        )}
      </div>
    );
  }

  if (cloud.account) return <AccountDetails account={cloud.account} />;

  if (cloud.error && !cloud.loading) {
    const problem = describeProblem({ kind: "transcription", error: cloud.error }, undefined, locale);
    return (
      <Banner
        tone="error"
        actions={
          <>
            <Button size="sm" variant="primary" onClick={cloud.refresh}>
              <RotateCw className="size-3.5" />
              {t("problems.retry")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => api.cloudSignOut()}>
              {t("cloud.signOut")}
            </Button>
          </>
        }
      >
        {t(problem.key, problem.params)}
      </Banner>
    );
  }

  return (
    <p className="flex items-center gap-2 text-[13px] text-text-3" role="status">
      <LoaderCircle className="size-4 animate-spin" />
      {t("cloud.loading")}
    </p>
  );
}
