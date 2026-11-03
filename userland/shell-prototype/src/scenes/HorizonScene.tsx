import { memo } from 'react';

/**
 * HORIZON: the eclipse ring over a violet wave landscape (owner's main
 * reference "AWA OS: A New Reality"). Drawn as one SVG so it stays sharp at
 * any resolution and costs no image memory. Text is never baked in.
 */

const W = 1600;
const H = 1000;
const RING = { cx: 800, cy: 430, r: 318 };
const FLOOR_Y = 720;

/** Left hill: high at the left edge, flowing down into the floor near the centre. */
const LEFT = { from: 0, to: 1200 };
const leftTop = (x: number) => 200 + Math.pow(x / 1200, 1.45) * 540 + Math.sin(x / 80) * 10 * (1 - x / 1200);
/** Right hill: lower, rising toward the right edge. */
const RIGHT = { from: 650, to: W };
const rightTop = (x: number) => 330 + Math.pow((W - x) / 950, 1.6) * 420 + Math.sin(x / 64) * 7 * ((W - x) / 950);

/** A contour `dy` below the hill top. Deeper lines ripple a little, like folds in fabric. */
function contour(range: { from: number; to: number }, f: (x: number) => number, dy: number, phase = 0) {
  const pts: string[] = [];
  // Step 14 px, always ending exactly on range.to so the shape meets the screen edge.
  const xs: number[] = [];
  for (let x = range.from; x < range.to; x += 14) xs.push(x);
  xs.push(range.to);
  for (const x of xs) {
    const y = f(x) + dy + Math.sin((x + phase * 37) / 70) * Math.min(14, dy * 0.06) + Math.sin((x + phase * 13) / 23) * Math.min(3, dy * 0.012);
    pts.push(`${x.toFixed(0)},${y.toFixed(1)}`);
  }
  return `M${pts.join(' L')}`;
}

const shape = (range: { from: number; to: number }, f: (x: number) => number) => `${contour(range, f, 0)} L${range.to},${H} L${range.from},${H} Z`;

const STARS = (() => {
  let s = 11;
  const r = () => ((s = (s * 16807) % 2147483647) / 2147483647);
  return Array.from({ length: 70 }, () => ({ x: r() * W, y: r() * 420, o: 0.15 + r() * 0.5, d: r() < 0.12 ? 1.6 : 1 }));
})();

/** One hill: fill, fine mesh, layered folds with soft glow, and a bright crest. */
function Hill({ id, range, f, lines, folds, fade }: { id: string; range: { from: number; to: number }; f: (x: number) => number; lines: number; folds: number[]; fade: 'right' | 'left' }) {
  return (
    <g mask={`url(#hz-fade-${fade})`}>
      <path d={shape(range, f)} fill="url(#hz-hill)" />
      <g clipPath={`url(#hz-clip-${id})`} fill="none">
        {Array.from({ length: lines }, (_, i) => (
          <path key={i} d={contour(range, f, 6 + i * 11, i)} stroke="url(#hz-mesh)" strokeWidth="0.7" opacity={0.9 - (i / lines) * 0.7} />
        ))}
        {folds.map((dy, i) => (
          <g key={dy}>
            <path d={contour(range, f, dy, 40 + i * 9)} stroke="#a855f7" strokeWidth="8" opacity="0.22" filter="url(#hz-blur-m)" />
            <path d={contour(range, f, dy, 40 + i * 9)} stroke="#d8b4fe" strokeWidth="1.3" opacity={0.55 - i * 0.12} />
          </g>
        ))}
      </g>
      <path d={contour(range, f, 0)} stroke="#a855f7" strokeWidth="14" fill="none" opacity="0.5" filter="url(#hz-blur-m)" />
      <path d={contour(range, f, 0)} stroke="#f3e8ff" strokeWidth="2.2" fill="none" />
    </g>
  );
}

export const HorizonScene = memo(function HorizonScene({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="hz-sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#060820" />
          <stop offset="0.55" stopColor="#0b0b30" />
          <stop offset="1" stopColor="#140a3c" />
        </linearGradient>
        <radialGradient id="hz-disc" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#1d1858" />
          <stop offset="0.85" stopColor="#100d3a" />
          <stop offset="1" stopColor="#2a1670" />
        </radialGradient>
        <radialGradient id="hz-halo" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0.62" stopColor="#a855f7" stopOpacity="0" />
          <stop offset="0.7" stopColor="#a855f7" stopOpacity="0.35" />
          <stop offset="1" stopColor="#a855f7" stopOpacity="0" />
        </radialGradient>
        <linearGradient id="hz-hill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#3b1d96" />
          <stop offset="0.5" stopColor="#1f0e5a" />
          <stop offset="1" stopColor="#120836" />
        </linearGradient>
        <linearGradient id="hz-floor" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#1c0c48" />
          <stop offset="1" stopColor="#0a0420" />
        </linearGradient>
        <linearGradient id="hz-mesh" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="#c084fc" stopOpacity="0.55" />
          <stop offset="1" stopColor="#8b5cf6" stopOpacity="0.25" />
        </linearGradient>
        <linearGradient id="hz-fade-l-grad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0.62" stopColor="#fff" />
          <stop offset="0.75" stopColor="#000" />
        </linearGradient>
        <linearGradient id="hz-fade-r-grad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0.42" stopColor="#000" />
          <stop offset="0.56" stopColor="#fff" />
        </linearGradient>
        <mask id="hz-fade-right" maskUnits="userSpaceOnUse" x="0" y="0" width={W} height={H}>
          <rect width={W} height={H} fill="url(#hz-fade-l-grad)" />
        </mask>
        <mask id="hz-fade-left" maskUnits="userSpaceOnUse" x="0" y="0" width={W} height={H}>
          <rect width={W} height={H} fill="url(#hz-fade-r-grad)" />
        </mask>
        <filter id="hz-blur-l" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="26" /></filter>
        <filter id="hz-blur-m" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="7" /></filter>
        <filter id="hz-blur-s" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="2.2" /></filter>
        <clipPath id="hz-clip-left"><path d={shape(LEFT, leftTop)} /></clipPath>
        <clipPath id="hz-clip-right"><path d={shape(RIGHT, rightTop)} /></clipPath>
      </defs>

      <rect width={W} height={H} fill="url(#hz-sky)" />
      {STARS.map((s, i) => <circle key={i} cx={s.x} cy={s.y} r={s.d} fill="#e9e2ff" opacity={s.o} />)}

      {/* the eclipse ring: wide halo, glow, then a white-hot core */}
      <circle cx={RING.cx} cy={RING.cy} r={RING.r * 1.45} fill="url(#hz-halo)" />
      <circle cx={RING.cx} cy={RING.cy} r={RING.r} fill="url(#hz-disc)" />
      <circle cx={RING.cx} cy={RING.cy} r={RING.r} fill="none" stroke="#9333ea" strokeWidth="46" opacity="0.55" filter="url(#hz-blur-l)" />
      <circle cx={RING.cx} cy={RING.cy} r={RING.r} fill="none" stroke="#c084fc" strokeWidth="14" opacity="0.9" filter="url(#hz-blur-m)" />
      <circle cx={RING.cx} cy={RING.cy} r={RING.r} fill="none" stroke="#e9d5ff" strokeWidth="6" filter="url(#hz-blur-s)" />
      <circle cx={RING.cx} cy={RING.cy} r={RING.r} fill="none" stroke="#fdf8ff" strokeWidth="2.5" />

      {/* reflective floor with the ring's glow on it */}
      <rect y={FLOOR_Y - 40} width={W} height={H - FLOOR_Y + 40} fill="url(#hz-floor)" />
      <ellipse cx={RING.cx} cy={FLOOR_Y + 40} rx="320" ry="30" fill="#a855f7" opacity="0.4" filter="url(#hz-blur-l)" />
      {Array.from({ length: 10 }, (_, i) => (
        <path key={i} d={`M${260 + i * 22},${FLOOR_Y + 14 + i * 26} Q800,${FLOOR_Y + 2 + i * 24} ${1340 - i * 22},${FLOOR_Y + 18 + i * 26}`} stroke="#8b5cf6" strokeOpacity={0.18 - i * 0.014} strokeWidth="1" fill="none" />
      ))}

      <Hill id="right" range={RIGHT} f={rightTop} lines={28} folds={[70, 150, 250]} fade="left" />
      <Hill id="left" range={LEFT} f={leftTop} lines={36} folds={[60, 135, 230, 340]} fade="right" />
    </svg>
  );
});
