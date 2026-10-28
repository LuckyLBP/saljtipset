import { type ButtonHTMLAttributes, type ReactNode, useEffect, useRef, useState } from "react";

type Variant = "primary" | "secondary" | "plain" | "danger";

const variants: Record<Variant, string> = {
  primary: "bg-honey text-on-accent hover:bg-honey-strong shadow-sm disabled:opacity-45",
  secondary: "bg-surface/80 text-ink border border-line hover:bg-surface shadow-[0_1px_2px_rgba(0,0,0,0.05)] disabled:opacity-45",
  plain: "text-ink hover:bg-hover disabled:opacity-40",
  danger: "bg-surface/80 text-danger border border-line hover:bg-danger-soft disabled:opacity-45",
};

/** Pill button. */
export function Button({
  variant = "secondary",
  size = "md",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: "sm" | "md" | "lg" }) {
  const sizes = { sm: "h-7 px-3 text-[12.5px]", md: "h-8 px-3.5 text-[13px]", lg: "h-10 px-5 text-[14px]" };
  return (
    <button
      {...props}
      className={`inline-flex shrink-0 items-center justify-center gap-1.5 rounded-full font-medium transition-[background,opacity,transform] active:scale-[0.97] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${sizes[size]} ${variants[variant]} ${className}`}
    />
  );
}

/** Round icon-only button. */
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
      className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted transition-colors hover:bg-hover hover:text-ink disabled:opacity-40 ${className}`}
    >
      {children}
    </button>
  );
}

/** iOS/macOS-style segmented control. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (v: T) => void;
}) {
  return (
    <div role="tablist" className="inline-flex rounded-full bg-sunken p-[3px]">
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            role="tab"
            aria-selected={active}
            onClick={() => onChange(o.value)}
            className={`h-7 rounded-full px-4 text-[12.5px] font-medium transition-all ${
              active ? "bg-surface text-ink shadow-[0_1px_3px_rgba(0,0,0,0.12),0_0_0_0.5px_rgba(0,0,0,0.04)] dark:bg-white/[0.16]" : "text-muted hover:text-ink"
            }`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

/** Small popover menu anchored to a trigger. */
export function Menu({
  trigger,
  items,
  align = "right",
}: {
  trigger: (open: () => void) => ReactNode;
  items: ({ label: string; icon?: ReactNode; danger?: boolean; disabled?: boolean; onSelect: () => void } | "divider")[];
  align?: "left" | "right";
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <div ref={ref} className="relative">
      {trigger(() => setOpen((o) => !o))}
      {open && (
        <div
          className={`glass rise absolute z-30 mt-1.5 min-w-52 rounded-2xl p-1.5 ${align === "right" ? "right-0" : "left-0"}`}
        >
          {items.map((it, i) =>
            it === "divider" ? (
              <div key={i} className="mx-2 my-1 h-px bg-line" />
            ) : (
              <button
                key={i}
                disabled={it.disabled}
                onClick={() => {
                  setOpen(false);
                  it.onSelect();
                }}
                className={`flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 text-left text-[13px] hover:bg-hover disabled:opacity-40 ${
                  it.danger ? "text-danger" : "text-ink"
                }`}
              >
                {it.icon && <span className="text-muted">{it.icon}</span>}
                {it.label}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}

export function ProgressBar({ value, indeterminate = false }: { value: number; indeterminate?: boolean }) {
  return (
    <div className="h-1 w-full overflow-hidden rounded-full bg-sunken" role="progressbar" aria-valuenow={Math.round(value)}>
      <div
        className={`h-full rounded-full bg-honey transition-[width] duration-300 ${indeterminate ? "w-1/3 animate-pulse" : ""}`}
        style={indeterminate ? undefined : { width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  );
}

export function Spinner({ className = "" }: { className?: string }) {
  return (
    <span
      className={`inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-r-transparent ${className}`}
      aria-hidden
    />
  );
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <div className={`rounded-2xl bg-surface shadow-[0_0_0_0.5px_var(--color-line),0_1px_3px_rgba(0,0,0,0.04)] ${className}`}>
      {children}
    </div>
  );
}

/** Button that briefly shows a confirmation label after its action succeeds. */
export function ConfirmButton({
  label,
  done,
  icon,
  onAction,
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick"> & {
  label: string;
  done: string;
  icon?: ReactNode;
  onAction: () => Promise<void>;
  variant?: Variant;
  size?: "sm" | "md" | "lg";
}) {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!shown) return;
    const t = setTimeout(() => setShown(false), 1800);
    return () => clearTimeout(t);
  }, [shown]);
  return (
    <Button
      {...props}
      onClick={async () => {
        await onAction();
        setShown(true);
      }}
    >
      {shown ? <Icon.Check /> : icon}
      {shown ? done : label}
    </Button>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <label className={`flex items-center justify-between gap-4 ${disabled ? "opacity-50" : ""}`}>
      <span>
        <span className="block text-[13px] font-medium">{label}</span>
        {hint && <span className="block text-[12px] text-muted">{hint}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors ${checked ? "bg-honey" : "bg-black/15 dark:bg-white/20"}`}
      >
        <span
          className={`absolute top-[2px] h-[18px] w-[18px] rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.25)] transition-[left] ${
            checked ? "left-[18px]" : "left-[2px]"
          }`}
        />
      </button>
    </label>
  );
}

export function ErrorNote({ children }: { children: ReactNode }) {
  return <div className="selectable rounded-xl bg-danger-soft px-3 py-2 text-[12.5px] text-danger">{children}</div>;
}

/** Status pill. */
export function Pill({ tone = "neutral", children }: { tone?: "neutral" | "accent" | "danger"; children: ReactNode }) {
  const tones = {
    neutral: "bg-sunken text-muted",
    accent: "bg-accent-soft text-accent",
    danger: "bg-danger-soft text-danger",
  };
  return <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11.5px] font-medium ${tones[tone]}`}>{children}</span>;
}

const svg = (d: ReactNode, size = 16) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
    {d}
  </svg>
);

/** SF Symbols-like line icons. */
export const Icon = {
  Plus: () => svg(<path d="M12 5v14M5 12h14" />),
  Copy: () => svg(<><rect x="8" y="8" width="12" height="12" rx="3" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></>),
  Share: () => svg(<><path d="M12 15V3m0 0-4 4m4-4 4 4" /><path d="M6 11H5a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-6a2 2 0 0 0-2-2h-1" /></>),
  More: () => svg(<><circle cx="5" cy="12" r="1.2" fill="currentColor" /><circle cx="12" cy="12" r="1.2" fill="currentColor" /><circle cx="19" cy="12" r="1.2" fill="currentColor" /></>),
  Trash: () => svg(<><path d="M4 7h16M10 11v6M14 11v6" /><path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" /></>),
  Pencil: () => svg(<path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z" />),
  Gear: () => svg(<><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" /></>),
  Chat: () => svg(<path d="M4 6.5A2.5 2.5 0 0 1 6.5 4h11A2.5 2.5 0 0 1 20 6.5v8a2.5 2.5 0 0 1-2.5 2.5H11l-4.5 3.5V17A2.5 2.5 0 0 1 4 14.5v-8Z" />),
  Sparkle: () => svg(<path d="M12 3c.5 4.5 2.5 6.5 7 7-4.5.5-6.5 2.5-7 7-.5-4.5-2.5-6.5-7-7 4.5-.5 6.5-2.5 7-7Z" />),
  Send: () => svg(<path d="M12 19V5m0 0-6 6m6-6 6 6" />, 17),
  Close: () => svg(<path d="M6 6l12 12M18 6 6 18" />, 14),
  Check: () => svg(<path d="m5 12.5 4.5 4.5L19 7" />, 15),
  Search: () => svg(<><circle cx="11" cy="11" r="6.5" /><path d="m20 20-4.2-4.2" /></>, 14),
  Wave: () => svg(<path d="M3 12h2m3-5v10m4-13v16m4-11v6m4-3h1" />),
  Download: () => svg(<path d="M12 4v11m0 0-4-4m4 4 4-4M5 20h14" />),
  Quote: () => svg(<path d="M7 7h4v4c0 3-1.5 5-4 6M15 7h4v4c0 3-1.5 5-4 6" />, 14),
  Refresh: () => svg(<><path d="M20 11a8 8 0 0 0-14.7-3.5M4 4v4h4" /><path d="M4 13a8 8 0 0 0 14.7 3.5M20 20v-4h-4" /></>, 14),
};
