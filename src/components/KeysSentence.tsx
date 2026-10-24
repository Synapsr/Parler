import { Fragment, type ReactNode } from "react";

/**
 * A translated sentence with keycaps in it: `{keys}` marks where they go and
 * `*…*` the words set in italics, so each language keeps its own word order.
 */
export function KeysSentence({ text, keys }: { text: string; keys: ReactNode }) {
  return text.split("{keys}").map((part, i) => (
    <Fragment key={i}>
      {i > 0 && keys}
      {part.split("*").map((run, j) =>
        j % 2 === 1 ? (
          <span key={j} className="italic">
            {run}
          </span>
        ) : (
          run
        ),
      )}
    </Fragment>
  ));
}
