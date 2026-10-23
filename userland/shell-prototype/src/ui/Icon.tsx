/** AWA's single stroke icon set: 24px grid, 1.7 stroke, round caps and joins. */
const PATHS = {
  home: 'M4 11l8-7 8 7v9a1 1 0 0 1-1 1h-4v-6H9v6H5a1 1 0 0 1-1-1z',
  box: 'M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  terminal: 'M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM7 10l3 2-3 2M12 15h5',
  browser: 'M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18',
  settings: 'M4 7h8M16 7h4M4 17h4M12 17h8M12 5v4M8 15v4',
  close: 'M6 6l12 12M18 6L6 18',
  minimize: 'M6 12h12',
  maximize: 'M6 6h12v12H6z',
  restore: 'M8 8h10v10H8zM6 16V6h10',
  search: 'M11 4a7 7 0 1 0 0 14a7 7 0 0 0 0-14zM20 20l-4-4',
  wifi: 'M2.5 9a14 14 0 0 1 19 0M6 12.5a9 9 0 0 1 12 0M9.3 16a4.3 4.3 0 0 1 5.4 0M12 19.5h.01',
  battery: 'M5 7h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2zM21 11v2',
  bluetooth: 'M7 7l10 10-5 4V3l5 4L7 17',
  shield: 'M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z',
  memory: 'M4 8h16v8H4zM8 8v8M12 8v8M16 8v8M6 16v3M18 16v3M6 5v3M18 5v3',
  gpu: 'M3 7h18v10H3zM7 11h4v2H7zM15 11h2M3 17v2M21 17v2',
  arrowRight: 'M5 12h14M13 6l6 6-6 6',
  download: 'M12 4v11M7 10l5 5 5-5M5 19h14',
  check: 'M5 12.5l4.5 4.5L19 7',
  alert: 'M12 4l9 16H3zM12 10v4M12 17h.01',
  plus: 'M12 5v14M5 12h14',
  reset: 'M4 12a8 8 0 1 0 2.4-5.7M4 4v4h4',
  play: 'M8 5v14l11-7z',
  lock: 'M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 7 0v3',
  chevronRight: 'M9 6l6 6-6 6',
  dataset: 'M4 5h16v14H4zM4 15l5-5 4 4 3-3 4 4',
  scene: 'M12 3l8 4.5v9L12 21l-8-4.5v-9z',
  image: 'M5 4h10l4 4v12H5zM15 4v4h4',
  doc: 'M5 4h10l4 4v12H5zM8 12h8M8 16h6',
  weights: 'M4 19V9M10 19V4M16 19v-7M22 19H2',
  frames: 'M4 5h16v14H4zM4 9h16M9 5v14',
  sun: 'M12 8a4 4 0 1 0 0 8a4 4 0 0 0 0-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4',
  volume: 'M4 9h4l5-4v14l-5-4H4zM16.5 8.5a5 5 0 0 1 0 7',
  feather: 'M20 4C11 4 6 9 6 18M6 18l-2 2M6 18h7a7 7 0 0 0 7-7V4',
  power: 'M12 3v8M6.3 6.3a8 8 0 1 0 11.4 0',
} as const;

export type IconName = keyof typeof PATHS;

type Props = { name: IconName; size?: number; stroke?: string; className?: string; strokeWidth?: number };

export function Icon({ name, size = 20, stroke = 'currentColor', className, strokeWidth = 1.7 }: Props) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={stroke}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
