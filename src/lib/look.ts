import { Cloud, Flame, Server, Sparkles, Zap, type LucideIcon } from "lucide-react";
import type { Tint } from "../ui";

/** How each service appears: an icon and a tint, never a borrowed logo. */
const LOOKS: Record<string, { icon: LucideIcon; tint: Tint }> = {
  cloud: { icon: Cloud, tint: "violet" },
  groq: { icon: Zap, tint: "amber" },
  openai: { icon: Sparkles, tint: "green" },
  fireworks: { icon: Flame, tint: "red" },
  custom: { icon: Server, tint: "blue" },
};

export const lookOf = (providerId: string) =>
  LOOKS[providerId] ?? { icon: Server, tint: "stone" as Tint };
