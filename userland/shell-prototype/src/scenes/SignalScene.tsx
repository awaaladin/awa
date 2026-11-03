import { memo } from 'react';

/**
 * SIGNAL (board 04, "Operating system"): a glowing ring at the centre with
 * fine inner and outer rings, a faint grid, circuit traces running to the
 * edges and glowing nodes. Text is never baked in.
 */

const W = 1600;
const H = 1000;
const C = { x: 800, y: 470 };
const R = 250;

/** Arc from angle a0 to a1 (degrees) on a circle of radius r. */
function arc(r: number, a0: number, a1: number) {
  const p = (a: number) => [C.x + r * Math.cos((a * Math.PI) / 180), C.y + r * Math.sin((a * Math.PI) / 180)];
  const [x0, y0] = p(a0);
  const [x1, y1] = p(a1);
  const large = Math.abs(a1 - a0) > 180 ? 1 : 0;
  return `M${x0!.toFixed(1)},${y0!.toFixed(1)} A${r},${r} 0 ${large} 1 ${x1!.toFixed(1)},${y1!.toFixed(1)}`;
}

// Circuit traces from the outer rings to the edges, mirrored into four corners.
const TRACES = [
  `M${C.x - 60},${C.y - 330} V${C.y - 400} H${C.x - 360} Q${C.x - 400},${C.y - 400} ${C.x - 400},${C.y - 360} V${C.y - 200} H${C.x - 760}`,
  `M${C.x + 60},${C.y - 330} V${C.y - 400} H${C.x + 360} Q${C.x + 400},${C.y - 400} ${C.x + 400},${C.y - 360} V${C.y - 200} H${C.x + 760}`,
  `M${C.x - 60},${C.y + 330} V${C.y + 420} H${C.x - 360} Q${C.x - 400},${C.y + 420} ${C.x - 400},${C.y + 380} V${C.y + 200} H${C.x - 760}`,
  `M${C.x + 60},${C.y + 330} V${C.y + 420} H${C.x + 360} Q${C.x + 400},${C.y + 420} ${C.x + 400},${C.y + 380} V${C.y + 200} H${C.x + 760}`,
  `M${C.x},${C.y - 350} V20`,
  `M${C.x},${C.y + 350} V${H - 20}`,
  `M${C.x - 350},${C.y} H40`,
  `M${C.x + 350},${C.y} H${W - 40}`,
  `M${C.x - 300},${C.y - 120} L${C.x - 520},${C.y - 120} L${C.x - 600},${C.y - 40} H${C.x - 780}`,
  `M${C.x + 300},${C.y + 120} L${C.x + 520},${C.y + 120} L${C.x + 600},${C.y + 40} H${C.x + 780}`,
];

const NODES: [number, number, number][] = [
  [C.x, C.y - 350, 4], [C.x, C.y + 350, 4], [C.x - 350, C.y, 4], [C.x + 350, C.y, 4],
  [C.x - 300, C.y - 230, 3], [C.x + 300, C.y - 230, 3], [C.x - 300, C.y + 230, 3], [C.x + 300, C.y + 230, 3],
  [C.x - 400, C.y - 200, 2.5], [C.x + 400, C.y + 200, 2.5], [C.x - 520, C.y - 120, 2], [C.x + 520, C.y + 120, 2],
];

export const SignalScene = memo(function SignalScene({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <radialGradient id="sg-bg" cx="0.5" cy="0.47" r="0.75">
          <stop offset="0" stopColor="#120c38" />
          <stop offset="0.55" stopColor="#070616" />
          <stop offset="1" stopColor="#030209" />
        </radialGradient>
        <pattern id="sg-grid" width="34" height="34" patternUnits="userSpaceOnUse">
          <path d="M34 0H0V34" fill="none" stroke="#6d5bd0" strokeWidth="0.6" opacity="0.16" />
        </pattern>
        <radialGradient id="sg-fade" cx="0.5" cy="0.47" r="0.6">
          <stop offset="0" stopColor="#fff" stopOpacity="0.9" />
          <stop offset="1" stopColor="#fff" stopOpacity="0.15" />
        </radialGradient>
        <mask id="sg-grid-mask"><rect width={W} height={H} fill="url(#sg-fade)" /></mask>
        <filter id="sg-blur-l" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="18" /></filter>
        <filter id="sg-blur-s" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="2.5" /></filter>
      </defs>
      <rect width={W} height={H} fill="url(#sg-bg)" />
      <rect width={W} height={H} fill="url(#sg-grid)" mask="url(#sg-grid-mask)" />

      <g fill="none" stroke="#8b7cf0" strokeWidth="1.2" opacity="0.5">
        {TRACES.map((d, i) => <path key={i} d={d} />)}
      </g>

      {/* outer rings and arc segments */}
      <g fill="none" stroke="#8b7cf0">
        <circle cx={C.x} cy={C.y} r={R + 100} strokeWidth="1" opacity="0.35" />
        <path d={arc(R + 70, 200, 340)} strokeWidth="2" opacity="0.55" />
        <path d={arc(R + 70, 20, 160)} strokeWidth="2" opacity="0.55" />
        <path d={arc(R + 48, 120, 240)} strokeWidth="1" opacity="0.35" />
        <path d={arc(R + 48, -60, 60)} strokeWidth="1" opacity="0.35" />
      </g>

      {/* the main ring */}
      <circle cx={C.x} cy={C.y} r={R} fill="#0a0820" />
      <circle cx={C.x} cy={C.y} r={R} fill="none" stroke="#9333ea" strokeWidth="36" opacity="0.45" filter="url(#sg-blur-l)" />
      <circle cx={C.x} cy={C.y} r={R} fill="none" stroke="#c084fc" strokeWidth="6" opacity="0.9" filter="url(#sg-blur-s)" />
      <circle cx={C.x} cy={C.y} r={R} fill="none" stroke="#f3e8ff" strokeWidth="2" />
      <circle cx={C.x} cy={C.y} r={R - 34} fill="none" stroke="#a78bfa" strokeWidth="1" opacity="0.5" />

      {NODES.map(([x, y, r], i) => (
        <g key={i}>
          <circle cx={x} cy={y} r={r * 4} fill="#a78bfa" opacity="0.25" filter="url(#sg-blur-s)" />
          <circle cx={x} cy={y} r={r} fill="#ede9fe" />
        </g>
      ))}
    </svg>
  );
});
