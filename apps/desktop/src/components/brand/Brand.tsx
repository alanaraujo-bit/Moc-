// The Mocó mark: an azulejo with a doorway — the little hideout — and the quarter-arc
// signature of the panel. Drawn on a 24-unit grid so it stays crisp at every size.

interface MarkProps {
  size?: number;
  className?: string;
  title?: string;
}

export function MocoMark({ size = 32, className, title = "Mocó" }: MarkProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className={className} role="img" aria-label={title}>
      <rect x="0.5" y="0.5" width="23" height="23" rx="5.5" fill="var(--mark-ground, var(--glaze-cobalt))" />
      <path d="M24 9.2 A9.2 9.2 0 0 1 14.8 0 H18.5 A5.5 5.5 0 0 1 24 5.5 Z" fill="var(--mark-arc, rgb(255 255 255 / 22%))" />
      <path d="M7.4 23.5 V13.6 a4.6 4.6 0 0 1 9.2 0 V23.5 Z" fill="var(--mark-door, #fff)" />
    </svg>
  );
}

export function Wordmark({ height = 20, className }: { height?: number; className?: string }) {
  // Set in the display face; the acute accent is the one flourish we allow ourselves.
  return (
    <span
      className={className}
      style={{
        fontFamily: "var(--font-display)",
        fontWeight: 650,
        fontSize: height,
        letterSpacing: "-0.035em",
        lineHeight: 1,
        color: "var(--ink)",
      }}
    >
      mocó
    </span>
  );
}
