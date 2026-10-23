/**
 * The dictation pill as drawn inside the app (onboarding, home), matching the
 * real overlay. `listening` dances, `writing` waves, `idle` rests.
 */
export function FlowPill({
  state = "listening",
  label,
}: {
  state?: "idle" | "listening" | "writing";
  label?: string;
}) {
  const heights = [8, 14, 22, 12, 18, 24, 10, 16, 20, 9, 15, 11];
  return (
    <span className="inline-flex h-11 items-center gap-2.5 rounded-full border border-white/10 bg-ink px-4 text-[12.5px] font-medium text-on-ink shadow-[0_16px_32px_-14px_rgb(0_0_0/0.6),inset_0_1px_0_rgb(255_255_255/0.06)]">
      {state === "writing" ? (
        <span className="size-2.5 animate-spin rounded-full border-[1.5px] border-white/25 border-t-white" />
      ) : (
        <span
          className={`size-2.5 rounded-full bg-[#ff5a3c] ${state === "listening" ? "breathe" : "opacity-60"}`}
        />
      )}
      <span className="flex h-6 items-center gap-[2px]">
        {heights.map((h, i) => (
          <span
            key={i}
            className={`w-[3px] rounded-full bg-on-ink ${state === "idle" ? "opacity-30" : "dance"} ${
              state === "writing" ? "opacity-55" : ""
            }`}
            style={{
              height: state === "idle" ? 3 : h,
              animationDelay: `${(i * 97) % 600}ms`,
              animationDuration: state === "writing" ? "1.2s" : `${0.7 + (i % 4) * 0.12}s`,
            }}
          />
        ))}
      </span>
      {label && <span className="pr-0.5">{label}</span>}
    </span>
  );
}
