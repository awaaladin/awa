import { memo } from 'react';

/**
 * FLOW (board 03, "Flow with intelligence"): hundreds of fine light strands
 * sweep in from the left, gather at a bright point on the upper right and
 * fall away toward the bottom right. Text is never baked in.
 */

const W = 1600;
const H = 1000;
const FOCUS = { x: 1250, y: 300 };

function strands() {
  let s = 5;
  const r = () => ((s = (s * 16807) % 2147483647) / 2147483647);
  const out: { d: string; o: number; w: number; c: number }[] = [];
  // Main sweep: from the left edge, arcing low, up through the focus and off the top right.
  for (let i = 0; i < 150; i++) {
    const t = i / 149;
    const y0 = 380 + t * 520 + (r() - 0.5) * 30;
    const sag = 640 + t * 260;
    const spread = (t - 0.5) * 70;
    const d = `M-40,${y0.toFixed(1)} C420,${(sag + spread).toFixed(1)} ${(FOCUS.x - 380).toFixed(1)},${(FOCUS.y + 260 + spread).toFixed(1)} ${(FOCUS.x + spread * 0.2).toFixed(1)},${(FOCUS.y + spread * 0.15).toFixed(1)} S${(1500 + t * 200).toFixed(1)},${(-60 - t * 40).toFixed(1)} ${(1520 + t * 160).toFixed(1)},-120`;
    out.push({ d, o: 0.18 + r() * 0.5, w: r() < 0.08 ? 1.4 : 0.6, c: t });
  }
  // Second fall: from the focus down to the bottom right.
  for (let i = 0; i < 90; i++) {
    const t = i / 89;
    const spread = (t - 0.5) * 120;
    const d = `M${(FOCUS.x - 40 + spread * 0.2).toFixed(1)},${(FOCUS.y + spread * 0.1).toFixed(1)} C${(FOCUS.x + 120 + spread).toFixed(1)},${(FOCUS.y + 160).toFixed(1)} ${(1320 + spread * 1.6).toFixed(1)},700 ${(1180 + t * 420).toFixed(1)},1080`;
    out.push({ d, o: 0.12 + r() * 0.4, w: r() < 0.06 ? 1.3 : 0.55, c: 0.5 + t * 0.5 });
  }
  return out;
}

const STRANDS = strands();

export const FlowScene = memo(function FlowScene({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <radialGradient id="fl-bg" cx="0.75" cy="0.35" r="0.85">
          <stop offset="0" stopColor="#140a3a" />
          <stop offset="0.5" stopColor="#07051a" />
          <stop offset="1" stopColor="#020208" />
        </radialGradient>
        <linearGradient id="fl-a" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#4c1d95" />
          <stop offset="0.55" stopColor="#8b5cf6" />
          <stop offset="0.8" stopColor="#d8b4fe" />
          <stop offset="1" stopColor="#7c3aed" />
        </linearGradient>
        <radialGradient id="fl-focus" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#f5edff" stopOpacity="0.95" />
          <stop offset="0.2" stopColor="#c084fc" stopOpacity="0.6" />
          <stop offset="1" stopColor="#7c3aed" stopOpacity="0" />
        </radialGradient>
        <filter id="fl-blur" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="6" /></filter>
      </defs>
      <rect width={W} height={H} fill="url(#fl-bg)" />
      {/* soft glow pass under the strands */}
      <g fill="none" stroke="#7c3aed" strokeWidth="5" opacity="0.18" filter="url(#fl-blur)">
        {STRANDS.filter((_, i) => i % 6 === 0).map((s, i) => <path key={i} d={s.d} />)}
      </g>
      <g fill="none" stroke="url(#fl-a)">
        {STRANDS.map((s, i) => <path key={i} d={s.d} strokeWidth={s.w} opacity={s.o} />)}
      </g>
      <circle cx={FOCUS.x} cy={FOCUS.y} r="150" fill="url(#fl-focus)" opacity="0.75" />
    </svg>
  );
});
