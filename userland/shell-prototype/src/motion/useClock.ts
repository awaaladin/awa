import { useEffect, useState } from 'react';

/** Current time, updated on each minute boundary. `?time=20:21` pins it (screenshots). */
export function useClock(): Date {
  const pinned = new URLSearchParams(location.search).get('time');
  const [now, setNow] = useState(() => pinnedDate(pinned) ?? new Date());
  useEffect(() => {
    if (pinned) return;
    let timer = 0;
    const schedule = () => {
      const d = new Date();
      timer = window.setTimeout(() => {
        setNow(new Date());
        schedule();
      }, 60_000 - (d.getSeconds() * 1000 + d.getMilliseconds()));
    };
    schedule();
    return () => window.clearTimeout(timer);
  }, [pinned]);
  return now;
}

function pinnedDate(v: string | null): Date | null {
  const m = v ? /^(\d{1,2}):(\d{2})$/.exec(v) : null;
  if (!m) return null;
  const d = new Date(2026, 9, 22, Number(m[1]), Number(m[2]));
  return d;
}

export const fmtTime = (d: Date) => d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
export const fmtShortDate = (d: Date) => d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }).replace(',', '');
export const fmtLongDate = (d: Date) => d.toLocaleDateString('en-GB', { weekday: 'long', day: 'numeric', month: 'long' });
