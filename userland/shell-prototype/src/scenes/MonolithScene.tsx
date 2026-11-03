import { memo } from 'react';

/**
 * MONOLITH (board 05, "Your world in sync"): a twilight sky, mountain
 * silhouettes, a translucent glass slab with one glowing edge, and its
 * reflection in still water. Text is never baked in.
 */

const W = 1600;
const H = 1000;
const HORIZON = 600;
// The slab: left edge vertical, top edge slanting up to the bright right edge.
const SLAB = { l: 650, r: 930, topL: 240, topR: 100, bottom: 596 };

const ridge = (pts: [number, number][]) => `M${pts.map(([x, y]) => `${x},${y}`).join(' L')} L${pts[pts.length - 1]![0]},${HORIZON + 4} L${pts[0]![0]},${HORIZON + 4} Z`;

const FAR = ridge([[0, 520], [90, 480], [180, 500], [260, 455], [340, 492], [430, 470], [520, 520], [640, 548], [960, 552], [1060, 520], [1180, 470], [1290, 490], [1400, 440], [1500, 470], [1600, 450]]);
const NEAR_L = ridge([[0, 545], [80, 520], [170, 540], [260, 525], [360, 560], [460, 580], [560, 596]]);
const NEAR_R = ridge([[1040, 596], [1160, 570], [1260, 540], [1360, 520], [1460, 495], [1600, 480]]);

export const MonolithScene = memo(function MonolithScene({ className }: { className?: string }) {
  const slab = `M${SLAB.l},${SLAB.topL} L${SLAB.r},${SLAB.topR} L${SLAB.r},${SLAB.bottom} L${SLAB.l},${SLAB.bottom} Z`;
  const mirror = (y: number) => 2 * SLAB.bottom - y + 12;
  const slabReflection = `M${SLAB.l},${mirror(SLAB.topL)} L${SLAB.r},${mirror(SLAB.topR)} L${SLAB.r},${SLAB.bottom + 12} L${SLAB.l},${SLAB.bottom + 12} Z`;
  return (
    <svg className={className} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="mn-sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#151463" />
          <stop offset="0.45" stopColor="#3730a3" />
          <stop offset="0.75" stopColor="#7c5cd6" />
          <stop offset="1" stopColor="#c4a5f5" />
        </linearGradient>
        <radialGradient id="mn-horizon-glow" cx="0.62" cy="0.6" r="0.45">
          <stop offset="0" stopColor="#e9d5ff" stopOpacity="0.55" />
          <stop offset="1" stopColor="#e9d5ff" stopOpacity="0" />
        </radialGradient>
        <linearGradient id="mn-slab" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#5b4fd6" stopOpacity="0.55" />
          <stop offset="0.6" stopColor="#3b2f9e" stopOpacity="0.7" />
          <stop offset="1" stopColor="#6d5ae6" stopOpacity="0.5" />
        </linearGradient>
        <linearGradient id="mn-water" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#6d5ad8" />
          <stop offset="0.35" stopColor="#3b2f9e" />
          <stop offset="1" stopColor="#120f4a" />
        </linearGradient>
        <linearGradient id="mn-reflect" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.55" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
        <mask id="mn-reflect-mask"><rect y={SLAB.bottom} width={W} height={H - SLAB.bottom} fill="url(#mn-reflect)" /></mask>
        <filter id="mn-blur-l" x="-50%" y="-20%" width="200%" height="140%"><feGaussianBlur stdDeviation="14" /></filter>
        <filter id="mn-blur-s" x="-50%" y="-20%" width="200%" height="140%"><feGaussianBlur stdDeviation="3" /></filter>
      </defs>

      <rect width={W} height={HORIZON + 10} fill="url(#mn-sky)" />
      <rect width={W} height={HORIZON + 10} fill="url(#mn-horizon-glow)" />

      <path d={FAR} fill="#4a3c9e" opacity="0.75" />
      <path d={NEAR_L} fill="#231c63" />
      <path d={NEAR_R} fill="#231c63" />

      {/* water */}
      <rect y={HORIZON} width={W} height={H - HORIZON} fill="url(#mn-water)" />
      <path d={`M0,${HORIZON + 40} Q300,${HORIZON + 30} 560,${HORIZON + 58} T1000,${HORIZON + 50} L1000,${HORIZON + 70} Q600,${HORIZON + 86} 0,${HORIZON + 110} Z`} fill="#1a155a" opacity="0.7" />
      <path d={`M0,${HORIZON + 150} Q200,${HORIZON + 130} 380,${HORIZON + 160} L380,${HORIZON + 190} Q160,${HORIZON + 200} 0,${HORIZON + 230} Z`} fill="#15114a" opacity="0.8" />

      {/* reflection of the slab and its bright edge */}
      <g mask="url(#mn-reflect-mask)">
        <path d={slabReflection} fill="url(#mn-slab)" opacity="0.6" />
        <rect x={SLAB.r - 2} y={SLAB.bottom + 10} width="4" height={H - SLAB.bottom} fill="#fff" opacity="0.8" filter="url(#mn-blur-s)" />
        <rect x={SLAB.r - 10} y={SLAB.bottom + 10} width="20" height={H - SLAB.bottom} fill="#c084fc" opacity="0.5" filter="url(#mn-blur-l)" />
      </g>
      {Array.from({ length: 14 }, (_, i) => (
        <path key={i} d={`M0,${HORIZON + 26 + i * 28} Q800,${HORIZON + 20 + i * 28} ${W},${HORIZON + 30 + i * 28}`} stroke="#c4b5fd" strokeOpacity={0.08 - i * 0.004} strokeWidth="1" fill="none" />
      ))}

      {/* the slab */}
      <path d={slab} fill="url(#mn-slab)" stroke="#a5b4fc" strokeOpacity="0.4" strokeWidth="1" />
      <path d={`M${SLAB.l},${SLAB.topL} L${SLAB.r},${SLAB.topR}`} stroke="#e0e7ff" strokeWidth="1.5" opacity="0.8" />
      <rect x={SLAB.r - 12} y={SLAB.topR} width="24" height={SLAB.bottom - SLAB.topR} fill="#c084fc" opacity="0.6" filter="url(#mn-blur-l)" />
      <line x1={SLAB.r} y1={SLAB.topR} x2={SLAB.r} y2={SLAB.bottom} stroke="#fff" strokeWidth="3" />
      <ellipse cx={SLAB.r} cy={SLAB.bottom + 4} rx="60" ry="8" fill="#e9d5ff" opacity="0.6" filter="url(#mn-blur-s)" />
    </svg>
  );
});
