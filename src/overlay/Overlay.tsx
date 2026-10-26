import { useEffect, useRef, useState } from "react";
import { api, events, type OverlayEvent, type OverlayPhase } from "../lib/api";
import { translate, resolveLocale, type Locale } from "../i18n";
import { describeProblem } from "../lib/problems";
import { sounds } from "./sounds";

const BARS = 18;
const MAX_HEIGHT = 24;

export function Overlay() {
  const [event, setEvent] = useState<OverlayEvent>({
    phase: "hidden",
    sounds: false,
    problem: null,
  });
  // The last visible phase, kept while the pill fades out.
  const [shown, setShown] = useState<OverlayPhase>("connecting");
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));
  const [locale, setLocale] = useState<Locale>("en");
  // A short message over the bars (Esc once more cancels), then gone.
  const [hint, setHint] = useState<string | null>(null);
  const hintTimer = useRef<number | undefined>(undefined);
  // A message kept until the backend takes it away (no sound from the
  // microphone); a passing hint covers it for a moment.
  const [kept, setKept] = useState<string | null>(null);
  const previous = useRef<OverlayPhase>("hidden");

  useEffect(() => {
    // The interface language can change while the pill is hidden.
    const loadLocale = () =>
      api
        .getSettings()
        .then((s) => setLocale(resolveLocale(s.uiLanguage)))
        .catch(() => {});
    loadLocale();
    const overlay = events.overlay((next) => {
      const before = previous.current;
      previous.current = next.phase;
      if (before === "hidden" && next.phase !== "hidden") loadLocale();
      if (next.phase === "connecting") setLevels(Array(BARS).fill(0));
      if (next.sounds) {
        if (next.phase === "recording" && before !== "recording") sounds.start();
        if (next.phase === "transcribing" && before === "recording") sounds.stop();
        if (next.phase === "error") sounds.error();
      }
      if (next.phase !== "hidden") setShown(next.phase);
      if (next.phase !== before) {
        window.clearTimeout(hintTimer.current);
        setHint(null);
        setKept(null);
      }
      setEvent(next);
    });
    const hinted = events.overlayHint((next) => {
      if (next.sounds) sounds.hint();
      const text = next.hint && `overlay.${next.hint}`;
      if (next.durationMs === null) {
        setKept(text);
        return;
      }
      setHint(text);
      window.clearTimeout(hintTimer.current);
      hintTimer.current = window.setTimeout(() => setHint(null), next.durationMs);
    });
    const level = events.level((value) => {
      setLevels((current) => [...current.slice(1), value]);
    });
    return () => {
      overlay.then((unlisten) => unlisten());
      level.then((unlisten) => unlisten());
      hinted.then((unlisten) => unlisten());
      window.clearTimeout(hintTimer.current);
    };
  }, []);

  const t = (key: string) => translate(locale, key);
  const phase = shown;
  const hidden = event.phase === "hidden";

  const shownHint = hint ?? kept;
  let message: string | null = shownHint ? t(shownHint) : null;
  if (phase === "empty") message = t("overlay.empty");
  if (phase === "error") {
    message = event.problem
      ? t(describeProblem(event.problem).short)
      : t("overlay.error");
  }

  return (
    <div className={`pill ${phase} ${shownHint ? "hinted" : ""} ${hidden ? "out" : ""}`}>
      {phase === "transcribing" ? <span className="spinner" /> : <span className="dot" />}
      {message ? (
        <span key={message} className="message">
          {message}
        </span>
      ) : (
        <span className="bars">
          {levels.map((level, i) => (
            <span
              key={i}
              className="bar"
              style={
                phase === "transcribing"
                  ? { animationDelay: `${i * 55}ms` }
                  : { height: `${Math.max(3, level * MAX_HEIGHT)}px` }
              }
            />
          ))}
        </span>
      )}
    </div>
  );
}
