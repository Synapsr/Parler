import { Check, ChevronDown, LoaderCircle } from "lucide-react";
import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from "react";

type ButtonVariant = "primary" | "accent" | "ghost" | "outline";

const BUTTON_STYLES: Record<ButtonVariant, string> = {
  primary:
    "bg-ink text-paper hover:bg-ink/85 disabled:bg-ink/30 shadow-[0_1px_0_rgb(0_0_0/0.2)]",
  accent:
    "bg-accent text-accent-ink hover:brightness-105 disabled:opacity-40 shadow-[0_1px_0_rgb(0_0_0/0.15)]",
  ghost: "text-ink-soft hover:bg-ink/5 hover:text-ink disabled:opacity-40",
  outline:
    "border border-line-strong text-ink hover:bg-ink/5 disabled:opacity-40",
};

export function Button({
  variant = "outline",
  size = "md",
  busy = false,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: "sm" | "md" | "lg";
  busy?: boolean;
}) {
  const sizes = {
    sm: "h-8 px-3 text-[13px] gap-1.5",
    md: "h-9 px-4 text-sm gap-2",
    lg: "h-12 px-6 text-[15px] gap-2",
  };
  return (
    <button
      {...props}
      disabled={props.disabled || busy}
      className={`inline-flex shrink-0 items-center justify-center rounded-full font-medium transition-[background,color,filter,opacity] duration-150 ${sizes[size]} ${BUTTON_STYLES[variant]} ${className}`}
    >
      {busy && <LoaderCircle className="size-4 animate-spin" />}
      {children}
    </button>
  );
}

export function IconButton({
  label,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      {...props}
      aria-label={label}
      title={label}
      className={`inline-flex size-8 items-center justify-center rounded-full text-muted transition-colors hover:bg-ink/5 hover:text-ink ${className}`}
    >
      {children}
    </button>
  );
}

/** Keys drawn as physical caps. */
export function Keycaps({
  keys,
  size = "md",
}: {
  keys: string[];
  size?: "md" | "lg";
}) {
  const cap =
    size === "lg"
      ? "min-w-[1.6em] h-[1.45em] px-[0.4em] text-[0.62em] rounded-[0.28em] border-b-[0.12em]"
      : "min-w-6 h-6 px-1.5 text-xs rounded-md border-b-2";
  return (
    <span className="inline-flex items-center gap-[0.18em] align-middle">
      {keys.map((key, i) => (
        <kbd
          key={`${key}-${i}`}
          className={`inline-flex items-center justify-center border border-line-strong bg-raised font-mono font-medium text-ink not-italic shadow-[inset_0_-1px_0_var(--line)] ${cap}`}
        >
          {key}
        </kbd>
      ))}
    </span>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <div className="inline-flex self-start rounded-full border border-line bg-sunken p-0.5">
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            aria-pressed={active}
            className={`h-7 rounded-full px-3 text-[13px] font-medium transition-all duration-200 ${
              active
                ? "bg-raised text-ink shadow-[0_1px_2px_rgb(0_0_0/0.12)]"
                : "text-muted hover:text-ink"
            }`}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-10 shrink-0 rounded-full transition-colors duration-200 ${
        checked ? "bg-accent" : "bg-ink/15"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow-sm transition-transform duration-200 ${
          checked ? "translate-x-4" : ""
        }`}
      />
    </button>
  );
}

export function TextInput({
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      spellCheck={false}
      autoComplete="off"
      autoCorrect="off"
      {...props}
      className={`h-10 w-full rounded-xl border border-line-strong bg-raised px-3.5 text-sm text-ink placeholder:text-muted/70 transition-colors focus:border-ink/40 focus:outline-none ${className}`}
    />
  );
}

export function Select({
  className = "",
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className={`relative ${className}`}>
      <select
        {...props}
        className="h-10 w-full appearance-none rounded-xl border border-line-strong bg-raised pr-9 pl-3.5 text-sm text-ink transition-colors focus:border-ink/40 focus:outline-none"
      >
        {children}
      </select>
      <ChevronDown className="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-muted" />
    </div>
  );
}

/** A titled group of settings. */
export function Section({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="rise">
      <h2 className="mb-2 px-1 font-display text-[22px] leading-tight text-ink">
        {title}
      </h2>
      <div className="divide-y divide-line rounded-2xl border border-line bg-raised/70">
        {children}
      </div>
    </section>
  );
}

/** One setting: a label and hint on the left, its control on the right or
 * below. */
export function Row({
  label,
  hint,
  children,
  stacked = false,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  stacked?: boolean;
}) {
  return (
    <div
      className={`flex gap-3 px-4 py-3.5 ${
        stacked ? "flex-col" : "items-center justify-between"
      }`}
    >
      <div className="min-w-0">
        <div className="text-sm font-medium text-ink">{label}</div>
        {hint && <div className="mt-0.5 text-[13px] text-muted">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

export function Success({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[13px] font-medium text-ok">
      <Check className="size-4" />
      {children}
    </span>
  );
}
