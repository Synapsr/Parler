import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  api,
  CLOUD,
  events,
  type AppInfo,
  type CloudAccount,
  type HistoryEntry,
  type Provider,
  type Settings,
  type Stats,
  type Status,
  type TranscribeError,
} from "./lib/api";

export interface Snapshot {
  info: AppInfo;
  settings: Settings;
  providers: Provider[];
  status: Status;
  history: HistoryEntry[];
  stats: Stats;
}

/** The Parler Cloud account of this computer. */
export interface CloudValue {
  signedIn: boolean;
  /** The last account the service sent, while signed in. */
  account: CloudAccount | null;
  loading: boolean;
  /** Why the last refresh failed, when there is no account to show. */
  error: TranscribeError | null;
  refresh: () => Promise<void>;
  /** The account a sign-in just returned. */
  received: (account: CloudAccount) => void;
}

interface ParlerValue extends Snapshot {
  provider: Provider | undefined;
  /** Saves a change; resolves to an error message when it was refused. */
  update: (patch: Partial<Settings>) => Promise<string | null>;
  cloud: CloudValue;
}

const ParlerContext = createContext<ParlerValue | null>(null);

export async function loadSnapshot(): Promise<Snapshot> {
  const [info, settings, providers, status, history, stats] = await Promise.all([
    api.appInfo(),
    api.getSettings(),
    api.getProviders(),
    api.getStatus(),
    api.getHistory(),
    api.getStats(),
  ]);
  return { info, settings, providers, status, history, stats };
}

export function ParlerProvider({
  initial,
  children,
}: {
  initial: Snapshot;
  children: ReactNode;
}) {
  const [settings, setSettings] = useState(initial.settings);
  const [status, setStatus] = useState(initial.status);
  const [history, setHistory] = useState(initial.history);
  const [stats, setStats] = useState(initial.stats);
  // Changes made in quick succession build on each other, not on a render
  // that hasn't happened yet.
  const latest = useRef(initial.settings);

  useEffect(() => {
    const subscriptions = [
      // Signing in or out of Parler Cloud changes the settings from the
      // backend: later saves build on that.
      events.settings((next) => {
        latest.current = next;
        setSettings(next);
      }),
      events.status(setStatus),
      events.history(setHistory),
      events.stats(setStats),
    ];
    return () => {
      subscriptions.forEach((s) => s.then((unlisten) => unlisten()));
    };
  }, []);

  const update = useCallback(async (patch: Partial<Settings>) => {
    const previous = latest.current;
    const next = { ...previous, ...patch };
    latest.current = next;
    setSettings(next);
    try {
      await api.saveSettings(next);
      return null;
    } catch (error) {
      // Only roll back if nothing newer was saved meanwhile.
      if (latest.current === next) {
        latest.current = previous;
        setSettings(previous);
      }
      return String(error);
    }
  }, []);

  const cloud = useCloudAccount(settings);

  const value = useMemo<ParlerValue>(
    () => ({
      info: initial.info,
      providers: initial.providers,
      settings,
      status,
      history,
      stats,
      provider: initial.providers.find((p) => p.id === settings.provider),
      update,
      cloud,
    }),
    [initial, settings, status, history, stats, update, cloud],
  );

  return (
    <ParlerContext.Provider value={value}>{children}</ParlerContext.Provider>
  );
}

/**
 * Follows the Parler Cloud account while it is the service: loaded at once,
 * then again whenever the window comes back to the front, so a plan bought
 * in the browser shows up.
 */
function useCloudAccount(settings: Settings): CloudValue {
  const token = settings.apiKeys[CLOUD]?.trim() ?? "";
  const signedIn = token !== "";
  const following = signedIn && settings.provider === CLOUD;
  const [account, setAccount] = useState<CloudAccount | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<TranscribeError | null>(null);
  const pending = useRef<Promise<void> | null>(null);

  const refresh = useCallback(() => {
    if (pending.current) return pending.current;
    setLoading(true);
    const request = api
      .cloudAccount()
      // `null`: the service no longer knows this computer; the backend has
      // forgotten the token and sends the new settings.
      .then((next) => {
        setAccount(next);
        setError(null);
      })
      .catch((e) => setError(e as TranscribeError))
      .finally(() => {
        setLoading(false);
        pending.current = null;
      });
    pending.current = request;
    return request;
  }, []);

  useEffect(() => {
    if (!following) {
      setAccount(null);
      setError(null);
      return;
    }
    refresh();
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
    });
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [following, token, refresh]);

  const received = useCallback((next: CloudAccount) => {
    setAccount(next);
    setError(null);
  }, []);

  return useMemo(
    () => ({ signedIn, account, loading, error, refresh, received }),
    [signedIn, account, loading, error, refresh, received],
  );
}

export function useParler(): ParlerValue {
  const value = useContext(ParlerContext);
  if (!value) throw new Error("useParler outside ParlerProvider");
  return value;
}
