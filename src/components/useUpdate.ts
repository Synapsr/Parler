import { useCallback, useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, events, type UpdateError, type UpdateStatus } from "../lib/api";

/** Where updates stand, kept current by the backend. */
export function useUpdate() {
  const [status, setStatus] = useState<UpdateStatus>({ state: "idle" });

  useEffect(() => {
    api.updateStatus().then(setStatus).catch(() => {});
    const unlisten = events.update(setStatus);
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  const check = useCallback(() => {
    // At once, rather than when the backend's event arrives.
    setStatus((current) => (current.state === "disabled" ? current : { state: "checking" }));
    api.checkForUpdate().then(setStatus).catch(() => {});
  }, []);
  // Progress and failures arrive as events.
  const install = useCallback(() => {
    api.installUpdate().catch(() => {});
  }, []);

  return { status, check, install };
}

/** An update failure in plain words. */
export function useUpdateError(): (error: UpdateError) => string {
  const { t } = useT();
  return (error) =>
    error.kind === "failed" ? t("updates.failed", { detail: error.detail }) : t(`updates.${error.kind}`);
}

/** "42 %", in the interface language. */
export function usePercent(): (share: number) => string {
  const { locale } = useT();
  return (share) => new Intl.NumberFormat(locale, { style: "percent" }).format(share);
}
