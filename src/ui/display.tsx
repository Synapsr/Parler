import { AlertCircle, CheckCircle2, Globe, Info, X, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

/** A white surface: the building block of every screen. */
export function Card({
  children,
  className = "",
  padded = true,
}: {
  children: ReactNode;
  className?: string;
  padded?: boolean;
}) {
  return (
    <div
      className={`rounded-card bg-surface shadow-card ${padded ? "p-5" : ""} ${className}`}
    >
      {children}
    </div>
  );
}

export type Tint = "red" | "amber" | "green" | "blue" | "violet" | "stone";

const TINTS: Record<Tint, string> = {
  red: "bg-[#fde7e2] text-[#c4371f] dark:bg-[#3a1d17] dark:text-[#ff8a72]",
  amber: "bg-[#fbefd9] text-[#a3640f] dark:bg-[#352914] dark:text-[#f0b75a]",
  green: "bg-[#e0f1e6] text-[#23774a] dark:bg-[#17301f] dark:text-[#6fd39a]",
  blue: "bg-[#e1ebfb] text-[#2c5fb8] dark:bg-[#172438] dark:text-[#86b0f5]",
  violet: "bg-[#ece5fa] text-[#6a43c2] dark:bg-[#251d38] dark:text-[#b69cf5]",
  stone: "bg-[#ece9e2] text-[#57524a] dark:bg-[#2b2926] dark:text-[#c9c2b5]",
};

/** An icon on a soft colour tile; each part of the app keeps its tint. */
export function IconTile({
  icon: Icon,
  tint = "stone",
  size = "md",
}: {
  icon: LucideIcon;
  tint?: Tint;
  size?: "sm" | "md" | "lg";
}) {
  const box = { sm: "size-7 rounded-lg", md: "size-9 rounded-xl", lg: "size-12 rounded-2xl" }[size];
  const glyph = { sm: "size-3.5", md: "size-[18px]", lg: "size-6" }[size];
  return (
    <span className={`inline-flex shrink-0 items-center justify-center ${box} ${TINTS[tint]}`}>
      <Icon className={glyph} strokeWidth={1.9} />
    </span>
  );
}

/** Keys drawn as physical caps. `pressed` pushes them down; `fn` carries
 * its globe, as on Apple keyboards. */
export function Keycaps({
  keys,
  size = "md",
  pressed = false,
  dark = false,
}: {
  keys: string[];
  size?: "sm" | "md" | "lg" | "xl";
  pressed?: boolean;
  /** For use on ink surfaces. */
  dark?: boolean;
}) {
  const cap = {
    sm: "h-5 min-w-5 px-1 text-[10.5px] rounded-[5px]",
    md: "h-7 min-w-7 px-2 text-[12.5px] rounded-lg",
    lg: "h-10 min-w-10 px-3 text-[16px] rounded-[10px]",
    xl: "h-16 min-w-16 px-5 text-[24px] rounded-2xl",
  }[size];
  const depth = { sm: 2, md: 3, lg: 4, xl: 6 }[size];
  const colors = dark
    ? "bg-ink-2 text-on-ink border-white/12"
    : "bg-surface text-text border-border-strong";
  return (
    <span className="inline-flex items-center gap-1 align-middle">
      {keys.map((key, i) => (
        <kbd
          key={`${key}-${i}`}
          className={`inline-flex items-center justify-center border font-mono font-medium not-italic transition-all duration-150 ${cap} ${colors}`}
          style={{
            boxShadow: pressed
              ? `0 1px 0 var(--border-strong)`
              : `0 ${depth}px 0 var(--border-strong)`,
            transform: pressed ? `translateY(${depth - 1}px)` : undefined,
          }}
        >
          {key === "fn" ? (
            <span className="inline-flex items-center gap-[0.3em]">
              <Globe className="size-[0.95em]" strokeWidth={1.8} aria-hidden="true" />
              fn
            </span>
          ) : (
            key
          )}
        </kbd>
      ))}
    </span>
  );
}

export type Tone = "error" | "warning" | "success" | "info";

const BANNER: Record<Tone, { box: string; icon: LucideIcon; iconColor: string }> = {
  error: { box: "bg-accent-soft", icon: AlertCircle, iconColor: "text-accent" },
  warning: { box: "bg-warning-soft", icon: AlertCircle, iconColor: "text-warning" },
  success: { box: "bg-success-soft", icon: CheckCircle2, iconColor: "text-success" },
  info: { box: "bg-surface-2", icon: Info, iconColor: "text-text-3" },
};

/** A message that needs the user, with what they can do about it. */
export function Banner({
  tone,
  children,
  actions,
}: {
  tone: Tone;
  children: ReactNode;
  actions?: ReactNode;
}) {
  const style = BANNER[tone];
  const Icon = style.icon;
  return (
    <div className={`rise flex gap-3 rounded-card px-4 py-3.5 ${style.box}`}>
      <Icon className={`mt-0.5 size-[18px] shrink-0 ${style.iconColor}`} />
      <div className="flex min-w-0 flex-1 flex-col gap-2.5">
        <div className="text-[13.5px] leading-snug text-text">{children}</div>
        {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
      </div>
    </div>
  );
}

export type DotTone = "ready" | "listening" | "busy" | "attention";

/** Where Parler stands, as a coloured light. */
export function StatusDot({ tone }: { tone: DotTone }) {
  const color = {
    ready: "bg-success",
    listening: "bg-accent breathe",
    busy: "bg-warning animate-pulse",
    attention: "bg-accent",
  }[tone];
  return <span className={`inline-block size-2 shrink-0 rounded-full ${color}`} />;
}

export function Stat({
  icon,
  tint,
  value,
  label,
}: {
  icon: LucideIcon;
  tint: Tint;
  value: string;
  label: string;
}) {
  // Han and kana are about twice as wide as the display font's figures.
  const wide = (value.match(/[\u3040-\u30ff\u3400-\u9fff]/g) ?? []).length >= 4;
  return (
    <Card className="flex flex-col gap-3">
      <IconTile icon={icon} tint={tint} size="sm" />
      <div>
        <div
          className={`font-display leading-none whitespace-nowrap text-text tabular-nums ${wide ? "text-[24px]" : "text-[30px]"}`}
        >
          {value}
        </div>
        <div className="mt-1.5 text-[12.5px] text-text-3">{label}</div>
      </div>
    </Card>
  );
}

export function Chip({
  children,
  onRemove,
  removeLabel,
}: {
  children: ReactNode;
  onRemove?: () => void;
  removeLabel?: string;
}) {
  return (
    <span className="rise inline-flex h-8 items-center gap-1 rounded-full bg-surface-2 pr-1.5 pl-3.5 text-[13px] text-text shadow-card">
      {children}
      {onRemove && (
        <button
          type="button"
          onClick={onRemove}
          aria-label={removeLabel}
          className="inline-flex size-5 items-center justify-center rounded-full text-text-3 hover:bg-text/8 hover:text-text"
        >
          <X className="size-3.5" />
        </button>
      )}
    </span>
  );
}

/** Progress through a short flow. */
export function StepDots({ count, current }: { count: number; current: number }) {
  return (
    <div className="flex items-center gap-1.5" aria-hidden="true">
      {Array.from({ length: count }, (_, i) => (
        <span
          key={i}
          className={`h-1.5 rounded-full transition-all duration-500 ${
            i === current ? "w-6 bg-text" : i < current ? "w-1.5 bg-text/50" : "w-1.5 bg-text/15"
          }`}
        />
      ))}
    </div>
  );
}

export function PageHeader({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <header className="rise mb-6">
      <h1 className="font-display text-title text-text">{title}</h1>
      {subtitle && <p className="mt-2 max-w-[52ch] text-[14px] text-text-2">{subtitle}</p>}
    </header>
  );
}

export function SectionHeader({
  icon,
  tint,
  title,
  description,
  action,
}: {
  icon: LucideIcon;
  tint: Tint;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="mb-4 flex items-center gap-3">
      <IconTile icon={icon} tint={tint} />
      <div className="min-w-0 flex-1">
        <h2 className="text-[15px] font-semibold text-text">{title}</h2>
        {description && <p className="text-[12.5px] text-text-3">{description}</p>}
      </div>
      {action}
    </div>
  );
}

/** Bars of a voice, for empty states. */
export function VoiceBars({ className = "" }: { className?: string }) {
  const heights = [10, 18, 28, 16, 24, 12, 20, 8];
  return (
    <span className={`inline-flex items-center gap-1 ${className}`} aria-hidden="true">
      {heights.map((h, i) => (
        <span
          key={i}
          className="dance w-1 rounded-full bg-current"
          style={{ height: h, animationDelay: `${i * 110}ms`, animationDuration: "1.6s" }}
        />
      ))}
    </span>
  );
}
