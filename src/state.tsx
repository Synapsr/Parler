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
  events,
  type AppInfo,
  type HistoryEntry,
  type Provider,
  type Settings,
  type Stats,
  type Status,
} from "./lib/api";

export interface Snapshot {
  info: AppInfo;
  settings: Settings;
  providers: Provider[];
  status: Status;
  history: HistoryEntry[];
  stats: Stats;
}

interface ParlerValue extends Snapshot {
  provider: Provider | undefined;
  /** Saves a change; resolves to an error message when it was refused. */
  update: (patch: Partial<Settings>) => Promise<string | null>;
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
    }),
    [initial, settings, status, history, stats, update],
  );

  return (
    <ParlerContext.Provider value={value}>{children}</ParlerContext.Provider>
  );
}

export function useParler(): ParlerValue {
  const value = useContext(ParlerContext);
  if (!value) throw new Error("useParler outside ParlerProvider");
  return value;
}
