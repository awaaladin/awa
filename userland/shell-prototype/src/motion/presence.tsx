import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTheme } from '../theme/ThemeProvider';
import { spring, type SpringName } from '../tokens';
import { useSprings } from './spring';

type Props = {
  open: boolean;
  children: ReactNode;
  className?: string;
  springName?: SpringName;
  /** Where the panel comes from: small lift and scale, like the approved prototype. */
  from?: { scale: number; y: number };
  label?: string;
  role?: string;
  id?: string;
};

/**
 * Keeps a panel mounted while it animates out, and drives it with an
 * interruptible spring: toggling mid-animation reverses from wherever it is.
 */
export function Presence({ open, children, className, springName = 'panel', from = { scale: 0.97, y: 10 }, label, role, id }: Props) {
  const { calm } = useTheme();
  const [mounted, setMounted] = useState(open);
  const el = useRef<HTMLDivElement>(null);
  const { to } = useSprings(
    { p: 0 },
    spring(springName),
    ({ p }) => {
      const node = el.current;
      if (!node) return;
      node.style.opacity = String(Math.max(0, Math.min(1, p)));
      node.style.transform = `translateY(${(1 - p) * from.y}px) scale(${from.scale + (1 - from.scale) * p})`;
    },
    calm,
  );

  useEffect(() => {
    if (open) {
      setMounted(true);
    } else if (mounted) {
      to({ p: 0 }, () => setMounted(false));
    }
  }, [open]);

  useEffect(() => {
    // The spring keeps its live position and velocity, so reopening mid-close reverses smoothly.
    if (mounted && open) to({ p: 1 });
  }, [mounted, open]);

  if (!mounted) return null;
  return (
    <div ref={el} className={className} style={{ opacity: 0 }} role={role} aria-label={label} id={id}>
      {children}
    </div>
  );
}
