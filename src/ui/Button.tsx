import { LoaderCircle } from "lucide-react";
import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "accent";

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-primary text-on-primary hover:opacity-90 disabled:opacity-35 shadow-[0_1px_2px_rgb(0_0_0/0.18)]",
  secondary:
    "bg-surface text-text shadow-card hover:bg-surface-2 disabled:opacity-45",
  ghost: "text-text-2 hover:bg-text/5 hover:text-text disabled:opacity-40",
  accent:
    "bg-accent text-white hover:brightness-105 disabled:opacity-40 shadow-[0_1px_2px_rgb(0_0_0/0.18)]",
};

const SIZES = {
  sm: "h-8 px-3.5 text-[13px] gap-1.5",
  md: "h-10 px-4.5 text-sm gap-2",
  lg: "h-12 px-6 text-[15px] gap-2",
};

export function Button({
  variant = "secondary",
  size = "md",
  busy = false,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: keyof typeof SIZES;
  busy?: boolean;
}) {
  return (
    <button
      type="button"
      {...props}
      disabled={props.disabled || busy}
      className={`inline-flex shrink-0 items-center justify-center rounded-full font-medium whitespace-nowrap transition-[background,color,opacity,filter] duration-150 ${SIZES[size]} ${VARIANTS[variant]} ${className}`}
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
      type="button"
      {...props}
      aria-label={label}
      title={label}
      className={`inline-flex size-8 shrink-0 items-center justify-center rounded-full text-text-3 transition-colors hover:bg-text/6 hover:text-text ${className}`}
    >
      {children}
    </button>
  );
}
