import { Check, ChevronDown, type LucideIcon } from "lucide-react";
import {
  useState,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";
import { IconTile, type Tint } from "./display";

const FIELD =
  "w-full rounded-xl bg-surface-2 text-sm text-text shadow-[inset_0_0_0_1px_var(--border-strong)] transition-shadow placeholder:text-text-3 focus:shadow-[inset_0_0_0_1.5px_var(--text-2),0_0_0_4px_var(--border)] focus:outline-none";

/** A labelled setting. `inline` puts the control on the right. */
export function Field({
  label,
  hint,
  children,
  inline = false,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  inline?: boolean;
}) {
  return (
    <div
      className={`flex gap-3 py-3.5 first:pt-0 last:pb-0 ${
        inline ? "items-center justify-between" : "flex-col gap-2"
      }`}
    >
      <div className="min-w-0">
        <div className="text-[13.5px] font-medium text-text">{label}</div>
        {hint && <div className="mt-0.5 text-[12.5px] leading-snug text-text-3">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

export function Input({
  className = "",
  leading,
  trailing,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { leading?: ReactNode; trailing?: ReactNode }) {
  return (
    <div className="relative w-full">
      {leading && (
        <div className="pointer-events-none absolute top-1/2 left-3.5 flex -translate-y-1/2 items-center text-text-3">
          {leading}
        </div>
      )}
      <input
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        {...props}
        className={`h-11 px-3.5 ${leading ? "pl-10" : ""} ${trailing ? "pr-11" : ""} ${FIELD} ${className}`}
      />
      {trailing && (
        <div className="absolute top-1/2 right-1.5 flex -translate-y-1/2 items-center">
          {trailing}
        </div>
      )}
    </div>
  );
}

export function TextArea({
  className = "",
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={`resize-none px-3.5 py-3 ${FIELD} ${className}`} />;
}

export function Select({
  className = "",
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className={`relative ${className}`}>
      <select {...props} className={`h-10 appearance-none pr-9 pl-3.5 ${FIELD}`}>
        {children}
      </select>
      <ChevronDown className="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-text-3" />
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
        checked ? "bg-success" : "bg-text/15"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.25)] transition-transform duration-200 ${
          checked ? "translate-x-4" : ""
        }`}
      />
    </button>
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
    <div className="inline-flex self-start rounded-full bg-surface-2 p-1 shadow-[inset_0_0_0_1px_var(--border)]">
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            aria-pressed={active}
            className={`h-8 rounded-full px-4 text-[13px] font-medium transition-all duration-200 ${
              active ? "bg-surface text-text shadow-card" : "text-text-3 hover:text-text"
            }`}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

/** A large, friendly radio option. */
export function ChoiceCard({
  icon,
  tint,
  title,
  description,
  badge,
  selected,
  onSelect,
  delay = 0,
}: {
  icon: LucideIcon;
  tint: Tint;
  title: string;
  description: string;
  badge?: string;
  selected: boolean;
  onSelect: () => void;
  delay?: number;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      style={{ animationDelay: `${delay}ms` }}
      className={`rise flex w-full items-center gap-3.5 rounded-card bg-surface px-4 py-3 text-left transition-shadow duration-200 ${
        selected
          ? "shadow-[0_0_0_2px_var(--text),0_10px_24px_-14px_rgb(0_0_0/0.35)]"
          : "shadow-card hover:shadow-[0_0_0_1px_var(--border-strong)]"
      }`}
    >
      <IconTile icon={icon} tint={tint} />
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2 text-[14.5px] font-semibold text-text">
          {title}
          {badge && (
            <span className="rounded-full bg-success-soft px-2 py-0.5 text-[11px] font-medium text-success">
              {badge}
            </span>
          )}
        </span>
        <span className="block text-[12.5px] text-text-3">{description}</span>
      </span>
      <span
        className={`flex size-5 shrink-0 items-center justify-center rounded-full transition-colors ${
          selected ? "bg-text text-surface" : "shadow-[inset_0_0_0_1.5px_var(--border-strong)]"
        }`}
      >
        {selected && <Check className="size-3" strokeWidth={3} />}
      </span>
    </button>
  );
}

/** Settings most people never need, folded away. */
export function Disclosure({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="pt-1">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="inline-flex items-center gap-1 text-[13px] font-medium text-text-2 hover:text-text"
      >
        <ChevronDown
          className={`size-4 transition-transform duration-200 ${open ? "" : "-rotate-90"}`}
        />
        {label}
      </button>
      {open && <div className="rise mt-3 flex flex-col gap-4">{children}</div>}
    </div>
  );
}
