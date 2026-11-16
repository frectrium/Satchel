import { animate, motion, useMotionValue, useTransform } from "framer-motion";
import { useEffect, useState, type ReactNode } from "react";

/** Satchel's mark: a satchel whose flap is three stacked blocks. */
export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
      <path d="M11 8.5V7a5 5 0 0 1 10 0v1.5" fill="none" stroke="var(--brand)" strokeWidth="2.4" strokeLinecap="round" />
      <rect x="3.5" y="9" width="25" height="19" rx="6" fill="var(--brand)" />
      <path d="M3.5 15h25" stroke="var(--bg)" strokeWidth="1.6" opacity="0.5" />
      <rect x="9.5" y="17.5" width="4" height="4" rx="1.2" fill="var(--bg)" />
      <rect x="14" y="17.5" width="4" height="4" rx="1.2" fill="var(--bg)" opacity="0.8" />
      <rect x="18.5" y="17.5" width="4" height="4" rx="1.2" fill="var(--bg)" opacity="0.6" />
    </svg>
  );
}

export function Arrow() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 12h14M13 6l6 6-6 6" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Icon({ name, size = 20 }: { name: "upload" | "download" | "trash" | "lock" | "copy" | "check" | "cloud" | "laptop" | "file"; size?: number }) {
  const p: Record<string, ReactNode> = {
    upload: <><path d="M12 16V4M6 10l6-6 6 6" /><path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3" /></>,
    download: <><path d="M12 4v12M6 10l6 6 6-6" /><path d="M4 20h16" /></>,
    trash: <><path d="M4 7h16M10 11v6M14 11v6" /><path d="M6 7l1 13h10l1-13M9 7V4h6v3" /></>,
    lock: <><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></>,
    copy: <><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3" /></>,
    check: <path d="M5 12l5 5 9-10" />,
    cloud: <path d="M7 18a4 4 0 0 1-.6-7.96A6 6 0 0 1 18 9a4.5 4.5 0 0 1-.5 9z" />,
    laptop: <><rect x="4" y="5" width="16" height="11" rx="1.5" /><path d="M2 19h20" /></>,
    file: <><path d="M14 3H7a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V7z" /><path d="M14 3v4h4" /></>,
  };
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {p[name]}
    </svg>
  );
}

/** A number that counts smoothly to its new value. */
export function Counter({ value, format }: { value: number; format: (n: number) => string }) {
  const mv = useMotionValue(value);
  const text = useTransform(mv, (v) => format(v));
  const [shown, setShown] = useState(format(value));
  useEffect(() => text.on("change", setShown), [text]);
  useEffect(() => {
    const controls = animate(mv, value, { duration: 0.8, ease: EASE });
    return () => controls.stop();
  }, [mv, value]);
  return <span>{shown}</span>;
}

export function Segmented<T extends string>({ value, options, onChange, id }: {
  value: T; options: { value: T; label: ReactNode }[]; onChange: (v: T) => void; id: string;
}) {
  return (
    <div className="segmented" role="tablist">
      {options.map((o) => (
        <button key={o.value} type="button" role="tab" aria-selected={o.value === value}
          className={o.value === value ? "on" : ""} onClick={() => onChange(o.value)}>
          {o.value === value && <motion.span layoutId={`seg-${id}`} className="seg-pill" transition={{ type: "spring", stiffness: 500, damping: 38 }} />}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Card({ children, className = "", ...rest }: { children: ReactNode; className?: string } & React.HTMLAttributes<HTMLDivElement>) {
  return <div className={`card ${className}`} {...rest}>{children}</div>;
}

/** Ease-out curve used across the site. */
export const EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];

export const fadeUp = {
  initial: { opacity: 0, y: 14 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.45, ease: EASE },
};
