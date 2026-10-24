import { Check, Copy, Trash2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useT } from "../i18n/context";
import { api, type HistoryEntry } from "../lib/api";
import { dayLabel, groupByDay } from "../lib/insights";
import { Card, IconButton, VoiceBars } from "../ui";

/** Dictations grouped under "Today", "Yesterday", then dates. */
export function HistoryDays({
  entries,
  highlight = "",
}: {
  entries: HistoryEntry[];
  /** Text to mark in each entry, e.g. a search. */
  highlight?: string;
}) {
  const { locale } = useT();
  const now = Date.now();
  return (
    <>
      {groupByDay(entries).map((day) => (
        <div key={day.start} className="flex flex-col gap-2">
          <h3 className="px-1 text-[11.5px] font-semibold tracking-wide text-text-3 uppercase">
            {dayLabel(day.start, now, locale)}
          </h3>
          <Card padded={false} className="divide-y divide-border overflow-hidden">
            {day.entries.map((entry) => (
              <Row key={entry.id} entry={entry} locale={locale} highlight={highlight} />
            ))}
          </Card>
        </div>
      ))}
    </>
  );
}

export function EmptyHistory({ children }: { children: ReactNode }) {
  return (
    <Card className="flex flex-col items-center gap-3 py-10 text-center">
      <VoiceBars className="text-text-3" />
      <div>{children}</div>
    </Card>
  );
}

/** `text` with each case-insensitive occurrence of `query` marked. */
function marked(text: string, query: string): ReactNode {
  const needle = query.trim().toLowerCase();
  if (!needle) return text;
  const parts: ReactNode[] = [];
  const haystack = text.toLowerCase();
  let from = 0;
  for (let at = haystack.indexOf(needle); at >= 0; at = haystack.indexOf(needle, from)) {
    parts.push(text.slice(from, at));
    parts.push(
      <mark key={at} className="rounded-[3px] bg-warning-soft px-0.5 text-text">
        {text.slice(at, at + needle.length)}
      </mark>,
    );
    from = at + needle.length;
  }
  parts.push(text.slice(from));
  return parts;
}

function Row({
  entry,
  locale,
  highlight,
}: {
  entry: HistoryEntry;
  locale: string;
  highlight: string;
}) {
  const { t } = useT();
  const [copied, setCopied] = useState(false);
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(entry.id);

  const copy = async () => {
    await api.copyText(entry.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  };

  return (
    <div className="group flex items-start gap-4 px-4 py-3.5 transition-colors hover:bg-surface-2">
      <span className="w-16 shrink-0 pt-px font-mono text-[11.5px] whitespace-nowrap text-text-3">
        {time}
      </span>
      <p data-selectable className="min-w-0 flex-1 text-[14px] leading-relaxed text-text">
        {marked(entry.text, highlight)}
      </p>
      <div className="-my-1 flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton label={copied ? t("common.copied") : t("common.copy")} onClick={copy}>
          {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
        </IconButton>
        <IconButton label={t("common.delete")} onClick={() => api.deleteHistoryEntry(entry.id)}>
          <Trash2 className="size-4" />
        </IconButton>
      </div>
    </div>
  );
}
