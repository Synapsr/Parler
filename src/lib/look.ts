import { Flame, Laptop, Server, Sparkles, Zap, type LucideIcon } from "lucide-react";
import type { Tint } from "../ui";

/** How each service appears: an icon and a tint, never a borrowed logo.
 * Parler Cloud wears Parler's own mark (see ProviderMark). */
const LOOKS: Record<string, { icon: LucideIcon; tint: Tint }> = {
  groq: { icon: Zap, tint: "amber" },
  openai: { icon: Sparkles, tint: "green" },
  fireworks: { icon: Flame, tint: "red" },
  custom: { icon: Server, tint: "blue" },
  // On this computer: the machine itself, in the colour of its hardware.
  local: { icon: Laptop, tint: "stone" },
};

export const lookOf = (providerId: string) =>
  LOOKS[providerId] ?? { icon: Server, tint: "stone" as Tint };

/** Services whose name is interface text rather than the backend's label. */
const NAMED: Record<string, string> = {
  custom: "providers.customName",
  local: "providers.localName",
};

/** A service's name as shown: "Your server" and "On this computer" are translated. */
export function providerName(
  provider: { id: string; label: string } | undefined,
  t: (key: string) => string,
): string {
  if (!provider) return "";
  return provider.id in NAMED ? t(NAMED[provider.id]) : provider.label;
}
