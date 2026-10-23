import { type ReactNode } from 'react';

export function Chip({ children, tone = 'default' }: { children: ReactNode; tone?: 'default' | 'accent' | 'warn' | 'muted' }) {
  return <span className={`chip chip-${tone}`}>{children}</span>;
}

/** A labelled usage bar. `value` and `max` share a unit. */
