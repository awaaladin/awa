import { type ReactNode } from 'react';

export function Meter({ label, value, max, detail, tone = 'acc' }: { label: string; value: number; max: number; detail?: ReactNode; tone?: 'acc' | 'acc2' }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="meter">
      <div className="meter-track" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={max} aria-valuenow={value}>
        <div className={`meter-fill meter-${tone}`} style={{ transform: `scaleX(${pct / 100})` }} />
      </div>
      {detail && <div className="meter-detail">{detail}</div>}
    </div>
  );
}
