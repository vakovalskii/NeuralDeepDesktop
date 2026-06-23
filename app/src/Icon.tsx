// Inline stroke-icon set (Lucide-style, 24×24, stroke=currentColor).
// One source of truth so the UI stays emoji-free and consistent.

type Name =
  | "plus" | "folder" | "send" | "spark" | "image" | "check"
  | "brain" | "puzzle" | "pin" | "pinOff" | "tool" | "alert"
  | "search" | "chevron" | "dot" | "shield" | "box" | "gauge";

const P: Record<Name, JSX.Element> = {
  plus: <><path d="M12 5v14" /><path d="M5 12h14" /></>,
  folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />,
  send: <><path d="M12 19V5" /><path d="m6 11 6-6 6 6" /></>,
  spark: <path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />,
  image: <><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="9" cy="9" r="1.6" /><path d="m21 15-5-5L5 21" /></>,
  check: <path d="M20 6 9 17l-5-5" />,
  brain: <path d="M9 4a3 3 0 0 0-3 3 3 3 0 0 0-1 5 3 3 0 0 0 2 5 2.5 2.5 0 0 0 5 .5V5a2 2 0 0 0-3-1Zm6 0a3 3 0 0 1 3 3 3 3 0 0 1 1 5 3 3 0 0 1-2 5 2.5 2.5 0 0 1-5 .5" />,
  puzzle: <path d="M14 3a2 2 0 0 0-4 0v1H7a2 2 0 0 0-2 2v3H4a2 2 0 0 0 0 4h1v3a2 2 0 0 0 2 2h3v-1a2 2 0 0 1 4 0v1h3a2 2 0 0 0 2-2v-3h1a2 2 0 0 0 0-4h-1V6a2 2 0 0 0-2-2h-3z" />,
  pin: <><path d="M9 4h6l-1 6 3 3H7l3-3z" /><path d="M12 16v4" /></>,
  pinOff: <><path d="M9 4h6l-1 6 3 3H7l3-3z" /><path d="M12 16v4" /><path d="m4 4 16 16" /></>,
  tool: <path d="M14.5 5.5a3.5 3.5 0 0 0-4.9 4.4l-5.3 5.3a1.5 1.5 0 0 0 2.1 2.1l5.3-5.3a3.5 3.5 0 0 0 4.4-4.9l-2 2-1.6-1.6z" />,
  alert: <><path d="M12 9v4" /><path d="M12 17h.01" /><path d="M10.3 3.9 2 18a2 2 0 0 0 1.7 3h16.6a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" /></>,
  search: <><circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" /></>,
  chevron: <path d="m9 18 6-6-6-6" />,
  dot: <circle cx="12" cy="12" r="5" />,
  shield: <path d="M12 3 5 6v5c0 4 3 7 7 8 4-1 7-4 7-8V6z" />,
  box: <><path d="M3 7.5 12 3l9 4.5v9L12 21l-9-4.5z" /><path d="M3 7.5 12 12l9-4.5M12 12v9" /></>,
  gauge: <><path d="M12 13a2 2 0 1 0 0-4" /><path d="M12 21a9 9 0 1 0-9-9" /><path d="M12 9V5" /></>,
};

export function Icon({ name, size = 18, className, fill }: { name: Name; size?: number; className?: string; fill?: boolean }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {P[name]}
    </svg>
  );
}
