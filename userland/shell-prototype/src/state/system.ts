/**
 * The one mock data source. Every number on screen (taskbar, widget, Settings,
 * terminal, Boxes) is derived from this state, so they always agree.
 * Terms follow the AWA spec: memory classes, boxes, scheduler classes, and
 * "measured by AWA" (kernel counters) vs "declared by app" (libawa budget levels).
 */

export type MemoryClass = 'Required' | 'Recomputable' | 'Streamable';
export type SchedClass = 'Frame-deadline' | 'Interactive' | 'Batch';
export type BoxKind = 'vm' | 'light';
export type BoxState = 'running' | 'stopped' | 'starting';

export type BudgetLevel = { minGB: number; label: string };

export type BoxApp = {
  name: string;
  memClass: MemoryClass;
  /** Resident RAM measured by the kernel. */
  measuredGB: number;
  /** Part of measuredGB that is unlocked Recomputable memory (droppable). */
  droppableGB: number;
  /** Budget levels the app declared through libawa; absent for apps that declare nothing. */
  levels?: BudgetLevel[];
  /** What the app itself reports using (libawa); absent for apps that report nothing. */
  selfReportGB?: number;
};

export type Box = {
  id: string;
  kind: BoxKind;
  state: BoxState;
  persistent: boolean;
  network: 'denied' | 'granted';
  limitGB: number;
  apps: BoxApp[];
};

export type GpuJob = { id: string; box: string; label: string; sched: SchedClass; vramGB: number; progress: number };

export type Download = {
  id: string;
  file: string;
  from: string;
  url: string;
  sizeGB: number;
  /** pending = waiting for the owner to pick a box in the broker. */
  state: 'pending' | 'downloading' | 'done' | 'canceled';
  box: string | null;
  received: number;
  connections: number;
};

export type FileItem = { name: string; size: string; memClass: MemoryClass; icon: 'folder' | 'dataset' | 'scene' | 'image' | 'doc' | 'weights' };

export type SystemState = {
  ramTotalGB: number;
  boxes: Box[];
  gpu: { name: string; vramTotalGB: number; displayGB: number; jobs: GpuJob[] };
  downloads: Download[];
  folders: Record<string, FileItem[]>;
  quick: { wifi: boolean; bluetooth: boolean; strictIsolation: boolean; volume: number; brightness: number };
  user: string;
};

export const initialSystem: SystemState = {
  ramTotalGB: 8,
  user: 'Awaaladin',
  boxes: [
    {
      id: 'system', kind: 'light', state: 'running', persistent: true, network: 'granted', limitGB: 1,
      apps: [
        { name: 'awa-shell', memClass: 'Required', measuredGB: 0.35, droppableGB: 0 },
        { name: 'services', memClass: 'Required', measuredGB: 0.25, droppableGB: 0 },
      ],
    },
    {
      id: 'render', kind: 'light', state: 'running', persistent: true, network: 'denied', limitGB: 3,
      apps: [
        {
          name: 'viewport', memClass: 'Recomputable', measuredGB: 0.8, droppableGB: 0.3, selfReportGB: 0.7,
          levels: [
            { minGB: 3, label: 'full-res preview, 32 samples' },
            { minGB: 1.5, label: 'half-res preview' },
            { minGB: 0.6, label: 'preview off' },
          ],
        },
      ],
    },
    {
      id: 'train', kind: 'light', state: 'running', persistent: true, network: 'denied', limitGB: 4,
      apps: [
        {
          name: 'trainer', memClass: 'Streamable', measuredGB: 0.5, droppableGB: 0, selfReportGB: 0.4,
          levels: [
            { minGB: 4, label: 'batch 32' },
            { minGB: 2, label: 'batch 16' },
            { minGB: 1, label: 'batch 4' },
          ],
        },
      ],
    },
    { id: 'scratch', kind: 'vm', state: 'stopped', persistent: false, network: 'denied', limitGB: 1, apps: [] },
  ],
  gpu: {
    name: 'GPU 0',
    vramTotalGB: 8,
    displayGB: 0.4,
    jobs: [
      { id: 'j1', box: 'render', label: 'render · frame 212 of 480', sched: 'Interactive', vramGB: 1.9, progress: 212 / 480 },
      { id: 'j2', box: 'train', label: 'train · epoch 6 of 20', sched: 'Batch', vramGB: 0.8, progress: 6 / 20 },
    ],
  },
  downloads: [],
  folders: {
    Home: [
      { name: 'Projects', size: '14 items', memClass: 'Required', icon: 'folder' },
      { name: 'imagenet-mini', size: '12 GB', memClass: 'Streamable', icon: 'dataset' },
      { name: 'shot-04.blend', size: '820 MB', memClass: 'Recomputable', icon: 'scene' },
      { name: 'render-0212.exr', size: '48 MB', memClass: 'Recomputable', icon: 'image' },
      { name: 'notes.md', size: '4 KB', memClass: 'Required', icon: 'doc' },
      { name: 'weights-v3.pt', size: '1.2 GB', memClass: 'Streamable', icon: 'weights' },
    ],
    Projects: [
      { name: 'viewport-tests', size: '6 items', memClass: 'Required', icon: 'folder' },
      { name: 'pose-estimation', size: '9 items', memClass: 'Required', icon: 'folder' },
    ],
    Datasets: [{ name: 'imagenet-mini', size: '12 GB', memClass: 'Streamable', icon: 'dataset' }],
    Renders: [{ name: 'render-0212.exr', size: '48 MB', memClass: 'Recomputable', icon: 'image' }],
    Downloads: [],
  },
  quick: { wifi: true, bluetooth: false, strictIsolation: true, volume: 0.62, brightness: 0.8 },
};

// ---------------------------------------------------------------- derived values

export const boxUsed = (b: Box) => round(b.apps.reduce((s, a) => s + a.measuredGB, 0));
export const boxDroppable = (b: Box) => round(b.apps.reduce((s, a) => s + a.droppableGB, 0));
export const ramUsed = (s: SystemState) => round(s.boxes.reduce((t, b) => t + boxUsed(b), 0));
export const vramUsed = (s: SystemState) => round(s.gpu.displayGB + s.gpu.jobs.reduce((t, j) => t + j.vramGB, 0));
export const boxClass = (b: Box): MemoryClass | '—' =>
  [...b.apps].sort((x, y) => y.measuredGB - x.measuredGB)[0]?.memClass ?? '—';
/** What apps say about themselves, and how many of them say anything. */
export function declared(s: SystemState) {
  const apps = s.boxes.flatMap((b) => b.apps);
  const reporting = apps.filter((a) => a.selfReportGB !== undefined);
  return { gb: round(reporting.reduce((t, a) => t + (a.selfReportGB ?? 0), 0)), reporting: reporting.length, total: apps.length };
}

export function currentLevel(app: BoxApp, limitGB: number): BudgetLevel | undefined {
  return app.levels?.find((l) => l.minGB <= limitGB + 1e-9) ?? app.levels?.[app.levels.length - 1];
}

export function round(x: number, places = 2) {
  const f = 10 ** places;
  return Math.round(x * f) / f;
}

export function fmtGB(x: number) {
  if (x > 0 && x < 1) return `${round(x * 1024, 0)} MB`;
  return `${round(x, 1).toFixed(1)} GB`;
}

/** Parses "3GB", "3G", "1.5gb", "512M", "512MB". Returns GB, or null if invalid. */
export function parseSize(text: string): number | null {
  const m = /^(\d+(?:\.\d+)?)\s*(g|gb|m|mb)$/i.exec(text.trim());
  if (!m || !m[1] || !m[2]) return null;
  const n = Number(m[1]);
  return m[2].toLowerCase().startsWith('m') ? n / 1024 : n;
}

// ---------------------------------------------------------------- tradeoff report

export type ReportLine = { source: 'measured' | 'declared'; text: string };

/**
 * What lowering (or raising) a box's limit costs, per the spec: kernel-measured
 * facts for every app, plus the app's own words only where it declared levels.
 */
export function tradeoff(box: Box, newGB: number): { lines: ReportLine[]; reclaimGB: number; ok: boolean } {
  const used = boxUsed(box);
  const lines: ReportLine[] = [];
  const reclaim = Math.max(0, round(used - newGB));
  if (newGB < 0.25) return { lines: [{ source: 'measured', text: 'A box needs at least 256 MB.' }], reclaimGB: 0, ok: false };
  if (reclaim === 0) {
    lines.push({ source: 'measured', text: `Uses ${fmtGB(used)} now, so nothing has to be freed.` });
  } else {
    const drop = Math.min(reclaim, boxDroppable(box));
    const rest = round(reclaim - drop);
    if (drop > 0) lines.push({ source: 'measured', text: `${fmtGB(drop)} of rebuildable cache is dropped first and rebuilt when needed.` });
    if (rest > 0) {
      const streamable = box.apps.some((a) => a.memClass === 'Streamable');
      lines.push({
        source: 'measured',
        text: streamable
          ? `${fmtGB(rest)} of disk-backed pages are released and read again from disk when used: slower first access.`
          : `${fmtGB(rest)} is compressed in memory: a little more CPU when it is touched.`,
      });
    }
  }
  for (const app of box.apps) {
    if (!app.levels) continue;
    const before = currentLevel(app, box.limitGB);
    const after = currentLevel(app, newGB);
    if (before && after && before !== after) lines.push({ source: 'declared', text: `${app.name}: "${after.label}" (was "${before.label}")` });
    else if (after) lines.push({ source: 'declared', text: `${app.name}: stays at "${after.label}"` });
  }
  return { lines, reclaimGB: reclaim, ok: true };
}

// ---------------------------------------------------------------- reducer

export type Action =
  | { type: 'setLimit'; box: string; gb: number }
  | { type: 'newBox'; name: string; kind: BoxKind }
  | { type: 'boxStarted'; box: string }
  | { type: 'resetBox'; box: string }
  | { type: 'runApp'; box: string; app: string }
  | { type: 'startDownload'; url: string; box: string | null }
  | { type: 'routeDownload'; id: string; box: string }
  | { type: 'cancelDownload'; id: string }
  | { type: 'tick'; seconds: number }
  | { type: 'quick'; key: keyof SystemState['quick']; value: boolean | number };

const BOX_NAME = /^[a-z][a-z0-9-]{0,23}$/;
export const validBoxName = (n: string) => BOX_NAME.test(n);

let nextId = 1;

function fileFromUrl(url: string) {
  try {
    const u = new URL(url);
    return { file: decodeURIComponent(u.pathname.split('/').filter(Boolean).pop() ?? 'download'), from: u.hostname };
  } catch {
    return { file: 'download', from: 'unknown' };
  }
}

/** Mock sizes for known demo files; anything else is 120 MB. */
function mockSize(file: string) {
  if (/weights|model|\.bin$|\.pt$|\.safetensors$/i.test(file)) return 1.2;
  if (/\.zip$|\.tar/i.test(file)) return 0.6;
  return 0.12;
}

export function reduce(s: SystemState, a: Action): SystemState {
  switch (a.type) {
    case 'setLimit':
      return { ...s, boxes: s.boxes.map((b) => (b.id === a.box ? { ...b, limitGB: round(a.gb) } : b)) };
    case 'newBox':
      if (s.boxes.some((b) => b.id === a.name)) return s;
      return {
        ...s,
        boxes: [...s.boxes, { id: a.name, kind: a.kind, state: 'starting', persistent: false, network: 'denied', limitGB: 1, apps: [] }],
      };
    case 'boxStarted':
      return { ...s, boxes: s.boxes.map((b) => (b.id === a.box && b.state === 'starting' ? { ...b, state: 'running' } : b)) };
    case 'resetBox':
      return { ...s, boxes: s.boxes.map((b) => (b.id === a.box ? { ...b, apps: [], state: b.state === 'stopped' ? 'stopped' : 'running' } : b)) };
    case 'runApp':
      return {
        ...s,
        boxes: s.boxes.map((b) =>
          b.id === a.box
            ? { ...b, state: 'running', apps: [...b.apps, { name: a.app, memClass: 'Required', measuredGB: 0.1, droppableGB: 0 }] }
            : b,
        ),
      };
    case 'startDownload': {
      const { file, from } = fileFromUrl(a.url);
      const d: Download = {
        id: `d${nextId++}`, file, from, url: a.url, sizeGB: mockSize(file),
        state: a.box ? 'downloading' : 'pending', box: a.box, received: 0, connections: a.box ? 8 : 0,
      };
      return { ...s, downloads: [...s.downloads, d] };
    }
    case 'routeDownload':
      return { ...s, downloads: s.downloads.map((d) => (d.id === a.id ? { ...d, box: a.box, state: 'downloading', connections: 8 } : d)) };
    case 'cancelDownload':
      return { ...s, downloads: s.downloads.map((d) => (d.id === a.id ? { ...d, state: 'canceled', connections: 0 } : d)) };
    case 'tick': {
      // Mock link speed: 6 MB/s per connection.
      let changed = false;
      const downloads = s.downloads.map((d) => {
        if (d.state !== 'downloading') return d;
        changed = true;
        const received = Math.min(d.sizeGB, d.received + (d.connections * 6 * a.seconds) / 1024);
        return received >= d.sizeGB ? { ...d, received, state: 'done' as const, connections: 0 } : { ...d, received };
      });
      return changed ? { ...s, downloads } : s;
    }
    case 'quick':
      return { ...s, quick: { ...s.quick, [a.key]: a.value } };
  }
}
