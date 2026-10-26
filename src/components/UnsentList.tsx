import { CloudOff, RotateCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, type Unsent } from "../lib/api";
import { describeProblem } from "../lib/problems";
import { ago, clock } from "../lib/unsent";
import { useParler } from "../state";
import { Button, Card, IconButton, IconTile } from "../ui";

/**
 * Dictations that couldn't be sent, as quiet rows above the history: each
 * can be sent again (its text then goes to the clipboard) or deleted.
 */
export function UnsentList() {
  const { t } = useT();
  const { pending, status } = useParler();
  // The row being sent again, until the dictation is over or the row gone.
  const [sending, setSending] = useState<number | null>(null);
  useEffect(() => {
    if (status.state === "idle") setSending(null);
  }, [status.state]);
  useEffect(() => {
    if (!pending.some((unsent) => unsent.id === sending)) setSending(null);
  }, [pending, sending]);

  if (pending.length === 0) return null;
  const now = Date.now();
  const idle = status.state === "idle" && sending === null;

  return (
    <div className="rise flex flex-col gap-2">
      <Card padded={false} className="divide-y divide-border overflow-hidden">
        {pending.map((unsent) => (
          <Row
            key={unsent.id}
            unsent={unsent}
            now={now}
            sending={sending === unsent.id}
            canSend={idle}
            onRetry={() => {
              setSending(unsent.id);
              api.retryDictation(unsent.id);
            }}
          />
        ))}
      </Card>
      <p className="px-1 text-[12px] text-text-3">{t("history.unsent.note")}</p>
    </div>
  );
}

function Row({
  unsent,
  now,
  sending,
  canSend,
  onRetry,
}: {
  unsent: Unsent;
  now: number;
  sending: boolean;
  canSend: boolean;
  onRetry: () => void;
}) {
  const { t, locale } = useT();
  const reason = describeProblem(
    { kind: "transcription", error: unsent.error },
    t("providers.customName"),
    locale,
  );

  return (
    <div className="flex items-center gap-3.5 px-4 py-3">
      <IconTile icon={CloudOff} size="sm" />
      <div className="min-w-0 flex-1">
        <p className="text-[13.5px] text-text-2">
          {t("history.unsent.row", { ago: ago(unsent.id, now, locale), length: clock(unsent.durationMs) })}
        </p>
        <p className="mt-0.5 truncate text-[12px] text-text-3">{t(reason.short)}</p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Button size="sm" busy={sending} disabled={!canSend} onClick={onRetry}>
          {!sending && <RotateCw className="size-3.5" />}
          {t("problems.retry")}
        </Button>
        <IconButton
          label={t("common.delete")}
          disabled={sending}
          className="disabled:opacity-40"
          onClick={() => api.deletePending(unsent.id)}
        >
          <Trash2 className="size-4" />
        </IconButton>
      </div>
    </div>
  );
}
