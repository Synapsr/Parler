import { Search, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { EmptyHistory, HistoryDays } from "../components/HistoryList";
import { UnsentList } from "../components/UnsentList";
import { useT } from "../i18n/context";
import { api } from "../lib/api";
import { useParler } from "../state";
import { Button, Input, PageHeader } from "../ui";

/** Dictations drawn at a time; more on demand keeps long histories quick. */
const PAGE = 60;

/** Lower case, accents dropped: "Été" matches "ete". */
const fold = (text: string) =>
  text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

export function HistoryPage() {
  const { t, locale } = useT();
  const { history } = useParler();
  const [query, setQuery] = useState("");
  const [shown, setShown] = useState(PAGE);

  const matches = useMemo(() => {
    const needle = fold(query.trim());
    return needle ? history.filter((entry) => fold(entry.text).includes(needle)) : history;
  }, [history, query]);

  // A new search starts from the top.
  useEffect(() => setShown(PAGE), [query]);

  const count = new Intl.NumberFormat(locale).format(history.length);

  return (
    <>
      <PageHeader
        title={t("history.title")}
        subtitle={history.length === 1 ? t("history.countOne") : t("history.count", { n: count })}
      />

      {/* Unsent dictations first: they wait for the person. */}
      <div className="mb-5 empty:hidden">
        <UnsentList />
      </div>

      {history.length === 0 ? (
        <EmptyHistory>
          <p className="text-[14px] font-medium text-text">{t("home.empty")}</p>
          <p className="mt-1 text-[13px] text-text-3">{t("home.emptyHint")}</p>
        </EmptyHistory>
      ) : (
        <div className="flex flex-col gap-5">
          <div className="rise flex gap-2">
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("history.search")}
              aria-label={t("history.search")}
              leading={<Search className="size-4" />}
              trailing={
                query && (
                  <button
                    type="button"
                    onClick={() => setQuery("")}
                    aria-label={t("history.clearSearch")}
                    className="flex size-8 items-center justify-center rounded-lg text-text-3 hover:text-text"
                  >
                    <X className="size-4" />
                  </button>
                )
              }
            />
            <ClearAll />
          </div>

          {matches.length === 0 ? (
            <p className="rise py-10 text-center text-[14px] text-text-3">
              {t("history.noResults", { query: query.trim() })}
            </p>
          ) : (
            <div className="rise flex flex-col gap-5" style={{ animationDelay: "60ms" }}>
              <HistoryDays entries={matches.slice(0, shown)} highlight={query} />
              {matches.length > shown && (
                <Button className="self-center" onClick={() => setShown((n) => n + PAGE)}>
                  {t("history.showMore")}
                </Button>
              )}
            </div>
          )}
        </div>
      )}
    </>
  );
}

/** Deleting everything takes a second click, within a few seconds. */
function ClearAll() {
  const { t } = useT();
  const [armed, setArmed] = useState(false);

  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), 4000);
    return () => clearTimeout(timer);
  }, [armed]);

  return (
    <Button
      className="h-11"
      variant={armed ? "accent" : "secondary"}
      onClick={() => {
        if (armed) {
          api.clearHistory();
          setArmed(false);
        } else {
          setArmed(true);
        }
      }}
    >
      <Trash2 className="size-4" />
      {armed ? t("history.confirmClear") : t("history.clearAll")}
    </Button>
  );
}
