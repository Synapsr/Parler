import { CLOUD } from "../lib/api";
import { lookOf } from "../lib/look";
import { CloudMark, IconTile } from "../ui";

/** A service's mark: Parler's own for Parler Cloud, an icon tile for the others. */
export function ProviderMark({ providerId, size = "md" }: { providerId: string; size?: "sm" | "md" }) {
  if (providerId === CLOUD) return <CloudMark size={size === "sm" ? 28 : 36} />;
  const look = lookOf(providerId);
  return <IconTile icon={look.icon} tint={look.tint} size={size} />;
}
