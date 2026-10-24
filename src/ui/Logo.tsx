import { Cloud } from "lucide-react";

/** The Parler mark: the recording light and four bars of a voice. */
export function LogoMark({
  size = 28,
  live = false,
}: {
  size?: number;
  /** Bars move, as while listening. */
  live?: boolean;
}) {
  const bars = [
    { x: 442, y: 402, h: 220 },
    { x: 536, y: 322, h: 380 },
    { x: 630, y: 372, h: 280 },
    { x: 724, y: 432, h: 160 },
  ];
  return (
    <svg
      width={size}
      height={size}
      viewBox="100 100 824 824"
      aria-hidden="true"
      className="shrink-0"
    >
      <rect x="100" y="100" width="824" height="824" rx="190" fill="#1E1C19" />
      <rect
        x="100.5"
        y="100.5"
        width="823"
        height="823"
        rx="190"
        fill="none"
        stroke="rgb(255 255 255 / 0.08)"
      />
      <circle cx="308" cy="512" r="66" fill="#FF5A3C" />
      {bars.map((bar, i) => (
        <rect
          key={bar.x}
          x={bar.x}
          y={bar.y}
          width="58"
          height={bar.h}
          rx="29"
          fill="#F6F2EA"
          className={live ? "dance" : undefined}
          style={
            live
              ? { animationDelay: `${i * 120}ms`, transformBox: "fill-box" }
              : undefined
          }
        />
      ))}
    </svg>
  );
}

export function Wordmark({ className = "" }: { className?: string }) {
  return (
    <span className={`font-display text-[22px] leading-none text-text ${className}`}>
      Parler
    </span>
  );
}

/** Parler Cloud: the Parler mark with a cloud badge, so the service reads as
 * Parler's own rather than a third party's. Made for surface cards. */
export function CloudMark({ size = 36 }: { size?: number }) {
  const badge = Math.round(size * 0.46);
  const glyph = Math.round(badge * 0.64);
  return (
    <span
      className="relative inline-flex shrink-0 dark:shadow-[0_0_0_1px_rgb(255_255_255/0.14)]"
      style={{ width: size, height: size, borderRadius: size * 0.23 }}
    >
      <LogoMark size={size} />
      <span
        className="absolute inline-flex items-center justify-center rounded-full bg-accent text-white ring-2 ring-surface"
        style={{ width: badge, height: badge, right: -badge * 0.38, bottom: -badge * 0.38 }}
      >
        <Cloud fill="currentColor" strokeWidth={1.5} style={{ width: glyph, height: glyph }} />
      </span>
    </span>
  );
}
