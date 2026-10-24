import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import { useT } from "../i18n/context";
import { api, type CloudAccount, type DeviceStart, type TranscribeError } from "../lib/api";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";

export type SignInState =
  | { phase: "idle" }
  | { phase: "starting" }
  | { phase: "waiting"; start: DeviceStart }
  | { phase: "approved"; account: CloudAccount; start: DeviceStart }
  | { phase: "denied" }
  | { phase: "expired" }
  | { phase: "failed"; message: string };

/** Polls that may fail in a row before giving up: a network blip is fine. */
const MAX_FAILURES = 5;

/**
 * Linking this computer to Parler Cloud: the browser opens on a code, and
 * Parler asks every few seconds whether the person has authorized it.
 */
export function useCloudSignIn() {
  const { t, locale } = useT();
  const { cloud } = useParler();
  const [state, setState] = useState<SignInState>({ phase: "idle" });
  // Bumped to abandon a sign-in: its pending answers are then ignored.
  const session = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pollSoon = useRef<(() => void) | null>(null);

  const stop = useCallback(() => {
    session.current += 1;
    clearTimeout(timer.current);
    pollSoon.current = null;
  }, []);

  useEffect(() => stop, [stop]);

  // Back from the browser: no need to wait for the next poll.
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) pollSoon.current?.();
    });
    return () => {
      unlisten.then((unsubscribe) => unsubscribe()).catch(() => {});
    };
  }, []);

  const explain = (error: unknown) => {
    const problem = describeProblem(
      { kind: "transcription", error: error as TranscribeError },
      undefined,
      locale,
    );
    return t(problem.key, problem.params);
  };

  const start = async () => {
    stop();
    const id = session.current;
    setState({ phase: "starting" });
    let begun: DeviceStart;
    try {
      begun = await api.cloudSignInStart();
    } catch (error) {
      if (id === session.current) setState({ phase: "failed", message: explain(error) });
      return;
    }
    if (id !== session.current) return;
    setState({ phase: "waiting", start: begun });

    const deadline = Date.now() + begun.expiresIn * 1000;
    const interval = Math.max(1, begun.interval) * 1000;
    let failures = 0;
    let last = 0;
    let busy = false;

    const poll = async () => {
      if (id !== session.current || busy) return;
      clearTimeout(timer.current);
      if (Date.now() > deadline) {
        stop();
        setState({ phase: "expired" });
        return;
      }
      busy = true;
      last = Date.now();
      try {
        const answer = await api.cloudSignInPoll(begun.deviceCode);
        failures = 0;
        if (id !== session.current) return;
        if (answer.status === "approved") {
          stop();
          cloud.received(answer.account);
          setState({ phase: "approved", account: answer.account, start: begun });
          return;
        }
        if (answer.status !== "pending") {
          stop();
          setState({ phase: answer.status });
          return;
        }
      } catch (error) {
        failures += 1;
        if (failures >= MAX_FAILURES && id === session.current) {
          stop();
          setState({ phase: "failed", message: explain(error) });
          return;
        }
      } finally {
        busy = false;
      }
      if (id === session.current) timer.current = setTimeout(poll, interval);
    };

    // The service refuses polls closer together than `interval`.
    pollSoon.current = () => {
      if (Date.now() - last >= interval) poll();
    };
    timer.current = setTimeout(poll, interval);
  };

  return {
    state,
    start,
    cancel: () => {
      stop();
      setState({ phase: "idle" });
    },
    reopen: () => {
      if (state.phase === "waiting") openUrl(state.start.verificationUrl).catch(() => {});
    },
  };
}

export type CloudSignIn = ReturnType<typeof useCloudSignIn>;
