import { Check, LifeBuoy } from "lucide-react";
import { useState } from "react";
import { useT } from "../i18n/context";
import { api, CLOUD, type SendLogError } from "../lib/api";
import { useParler } from "../state";
import { Button, TextArea } from "../ui";

const SUPPORT_EMAIL = "parler@synapsr.io";

type State =
  | { step: "closed" }
  | { step: "writing"; error: SendLogError | null }
  | { step: "sending" }
  | { step: "sent"; reference: string };

/**
 * "Something wrong? Send the log": a quiet link that opens a one-field form.
 * The log goes to support through Parler Cloud, never a dictation or a key.
 */
export function SendLog() {
  const { t } = useT();
  const { settings, cloud } = useParler();
  const [state, setState] = useState<State>({ step: "closed" });
  const [note, setNote] = useState("");

  // Signed in to Parler Cloud: the answer goes to the account's address.
  const email = settings.apiKeys[CLOUD]?.trim() ? (cloud.account?.email ?? null) : null;

  const send = () => {
    setState({ step: "sending" });
    api
      .sendDiagnostics(note)
      .then((reference) => {
        setNote("");
        setState({ step: "sent", reference });
      })
      .catch((error: SendLogError) => setState({ step: "writing", error }));
  };

  if (state.step === "closed") {
    return (
      <button
        type="button"
        onClick={() => setState({ step: "writing", error: null })}
        className="inline-flex items-center gap-1.5 text-[13px] text-text-3 transition-colors hover:text-text"
      >
        <LifeBuoy className="size-3.5" />
        {t("settings.about.log.open")}
      </button>
    );
  }

  if (state.step === "sent") {
    return (
      <p className="rise inline-flex items-center gap-1.5 text-[13px] text-text-2">
        <Check className="size-4 text-success" />
        {t("settings.about.log.sent", { reference: state.reference })}
      </p>
    );
  }

  const error = state.step === "writing" ? state.error : null;
  return (
    <div className="rise flex w-full max-w-[460px] flex-col gap-2.5">
      <TextArea
        rows={3}
        autoFocus
        maxLength={4000}
        value={note}
        onChange={(e) => setNote(e.target.value)}
        placeholder={t(email ? "settings.about.log.placeholder" : "settings.about.log.placeholderEmail")}
        className="text-[13px]"
      />
      <p className="text-[12px] leading-relaxed text-text-3">
        {t("settings.about.log.privacy")}
        {email && ` ${t("settings.about.log.replyTo", { email })}`}
      </p>
      {error && (
        <p className="text-[12.5px] leading-relaxed text-accent-text">
          {error === "tooMany" ? t("settings.about.log.tooMany") : t("settings.about.log.failed", { email: SUPPORT_EMAIL })}{" "}
          {error !== "tooMany" && (
            <button type="button" onClick={() => void api.revealLog()} className="font-medium underline underline-offset-2">
              {t("settings.about.log.showFile")}
            </button>
          )}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={() => setState({ step: "closed" })} disabled={state.step === "sending"}>
          {t("common.cancel")}
        </Button>
        <Button size="sm" variant="primary" onClick={send} busy={state.step === "sending"}>
          {state.step === "sending" ? t("settings.about.log.sending") : t("settings.about.log.send")}
        </Button>
      </div>
    </div>
  );
}
