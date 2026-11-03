import { memo } from 'react';
import { logoIcon } from '../tokens';

/**
 * ORBIT: the AWA mark inside a glowing orbit ring with glass orbs (owner's
 * main reference "AWA OS: Beyond Limits"). The logo itself is used as-is;
 * only light is added around it. The ring passes behind the mark at the top
 * and in front of it at the bottom.
 */

const W = 1600;
const H = 1000;
const C = { x: 800, y: 500 };
const RX = 560;
const RY = 170;
const TILT = -16; // degrees

/** Half of the tilted orbit as a path: back = upper half, front = lower half. */
function arc(half: 'back' | 'front') {
  const pts: string[] = [];
  const [a0, a1] = half === 'back' ? [Math.PI, 2 * Math.PI] : [0, Math.PI];
  const t = (TILT * Math.PI) / 180;
  for (let i = 0; i <= 80; i++) {
    const a = a0 + ((a1 - a0) * i) / 80;
    const x = RX * Math.cos(a);
    const y = RY * Math.sin(a);
    pts.push(`${(C.x + x * Math.cos(t) - y * Math.sin(t)).toFixed(1)},${(C.y + x * Math.sin(t) + y * Math.cos(t)).toFixed(1)}`);
  }
  return `M${pts.join(' L')}`;
}

const ORBS = [
  { x: 560, y: 300, r: 24 },
  { x: 1100, y: 230, r: 18 },
];

/** The orbit is a fine line of light (board 02), brighter where it passes in front. */
function Ring({ d, front }: { d: string; front: boolean }) {
  return (
    <g fill="none" strokeLinecap="round">
      <path d={d} stroke="#8b5cf6" strokeWidth="12" opacity={front ? 0.35 : 0.2} filter="url(#ob-blur-l)" />
      <path d={d} stroke="#c4b5fd" strokeWidth="3" opacity={front ? 0.7 : 0.4} filter="url(#ob-blur-s)" />
      <path d={d} stroke="#f5f3ff" strokeWidth="1.2" opacity={front ? 0.95 : 0.55} />
    </g>
  );
}

export const OrbitScene = memo(function OrbitScene({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <radialGradient id="ob-bg" cx="0.52" cy="0.5" r="0.7">
          <stop offset="0" stopColor="#1c1252" />
          <stop offset="0.45" stopColor="#0c0a2a" />
          <stop offset="1" stopColor="#04030c" />
        </radialGradient>
        <radialGradient id="ob-glow" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#8b5cf6" stopOpacity="0.55" />
          <stop offset="1" stopColor="#8b5cf6" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="ob-orb" cx="0.35" cy="0.3" r="0.75">
          <stop offset="0" stopColor="#ffffff" stopOpacity="0.95" />
          <stop offset="0.25" stopColor="#c4b5fd" stopOpacity="0.7" />
          <stop offset="0.7" stopColor="#6d28d9" stopOpacity="0.45" />
          <stop offset="1" stopColor="#312e81" stopOpacity="0.2" />
        </radialGradient>
        <filter id="ob-blur-l" x="-20%" y="-50%" width="140%" height="200%"><feGaussianBlur stdDeviation="14" /></filter>
        <filter id="ob-blur-s" x="-20%" y="-50%" width="140%" height="200%"><feGaussianBlur stdDeviation="2" /></filter>
        <filter id="ob-logo-glow" x="-40%" y="-40%" width="180%" height="180%">
          <feGaussianBlur in="SourceAlpha" stdDeviation="28" result="b" />
          <feFlood floodColor="#a855f7" floodOpacity="0.75" />
          <feComposite in2="b" operator="in" result="g" />
          <feMerge><feMergeNode in="g" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>

      <rect width={W} height={H} fill="url(#ob-bg)" />
      <circle cx={C.x} cy={C.y} r="430" fill="url(#ob-glow)" />

      <Ring d={arc('back')} front={false} />
      <image href={logoIcon} x={C.x - 230} y={C.y - 262} width="460" height="498" filter="url(#ob-logo-glow)" />
      <Ring d={arc('front')} front />

      {ORBS.map((o, i) => (
        <g key={i}>
          <circle cx={o.x} cy={o.y} r={o.r * 1.8} fill="#8b5cf6" opacity="0.18" filter="url(#ob-blur-l)" />
          <circle cx={o.x} cy={o.y} r={o.r} fill="url(#ob-orb)" stroke="#e9d5ff" strokeOpacity="0.5" strokeWidth="1" />
          <ellipse cx={o.x - o.r * 0.3} cy={o.y - o.r * 0.38} rx={o.r * 0.32} ry={o.r * 0.18} fill="#fff" opacity="0.85" />
        </g>
      ))}
    </svg>
  );
});
