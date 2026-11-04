import { Plus } from "lucide-react";
import { useState, type FormEvent } from "react";
import { useLocalModels } from "../components/useLocalModels";
import { useT } from "../i18n/context";
import { LOCAL } from "../lib/api";
import { parseWords } from "../lib/vocabulary";
import { useParler } from "../state";
import { Button, Card, Chip, Input, PageHeader } from "../ui";

const EXAMPLES = ["Synapsr", "Kubernetes", "Plougastel", "Groq"];

export function Vocabulary() {
  const { t } = useT();
  const { settings, update } = useParler();
  const [draft, setDraft] = useState("");
  const words = parseWords(settings.vocabulary);
  // Parakeet transcribes without hints: say so rather than let words pile up unused.
  const local = useLocalModels();
  const model = settings.provider === LOCAL ? local.chosen : undefined;
  const other = local.models.find((m) => m.supportsVocabulary);
  const ignored = model && !model.supportsVocabulary && other;

  const save = (next: string[]) => update({ vocabulary: next.join(", ") });

  const add = (event?: FormEvent) => {
    event?.preventDefault();
    const added = parseWords(draft);
    if (added.length === 0) return;
    save(parseWords([...words, ...added].join(",")));
    setDraft("");
  };

  return (
    <>
      <PageHeader title={t("vocabulary.title")} subtitle={t("vocabulary.hint")} />
      {ignored && (
        <p className="rise mb-4 rounded-xl bg-warning-soft px-3.5 py-2.5 text-[12.5px] leading-snug text-text">
          {t("local.vocabularyIgnored", { model: model.name, other: other.name })}
        </p>
      )}
      <Card className="rise flex flex-col gap-5">
        <form onSubmit={add} className="flex gap-2">
          <Input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={t("vocabulary.placeholder")}
            autoFocus
          />
          <Button type="submit" variant="primary" disabled={!draft.trim()} className="h-11">
            <Plus className="size-4" />
            {t("vocabulary.add")}
          </Button>
        </form>

        {words.length > 0 ? (
          <div className="flex flex-col gap-3">
            <div className="flex flex-wrap gap-2">
              {words.map((word) => (
                <Chip
                  key={word}
                  removeLabel={t("vocabulary.remove", { word })}
                  onRemove={() => save(words.filter((w) => w !== word))}
                >
                  {word}
                </Chip>
              ))}
            </div>
            <p className="text-[12px] text-text-3">{t("vocabulary.count", { n: words.length })}</p>
          </div>
        ) : (
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[13px] text-text-3">{t("vocabulary.empty")}</span>
            {EXAMPLES.map((example) => (
              <button
                key={example}
                type="button"
                onClick={() => save([example])}
                className="inline-flex h-8 items-center gap-1 rounded-full px-3 text-[13px] text-text-2 shadow-[inset_0_0_0_1px_var(--border-strong)] hover:text-text"
              >
                <Plus className="size-3.5" />
                {example}
              </button>
            ))}
          </div>
        )}
      </Card>
    </>
  );
}
