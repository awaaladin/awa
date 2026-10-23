import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Presence } from '../motion/presence';
import { useSystem } from '../state/SystemProvider';
import { Icon, type IconName } from '../ui/Icon';
import { EmptyState } from '../ui/States';
import { APP_IDS, APPS, type OpenRequest } from './apps';

type Props = { open: boolean; onOpen: (r: OpenRequest) => void; onClose: () => void };

type Entry = { label: string; icon: IconName; hint?: string; request: OpenRequest };

const ENTRIES: Entry[] = [
  ...APP_IDS.map((id) => ({ label: APPS[id].title, icon: APPS[id].icon, request: { app: id } })),
  { label: 'Viewport', icon: 'scene', hint: 'in box "render"', request: { app: 'boxes' } },
  { label: 'Trainer', icon: 'weights', hint: 'in box "train"', request: { app: 'terminal' } },
  { label: 'Frames', icon: 'frames', hint: 'renders folder', request: { app: 'files' } },
  { label: 'Memory', icon: 'memory', hint: 'Settings', request: { app: 'settings', section: 'memory' } },
  { label: 'Appearance', icon: 'sun', hint: 'Settings', request: { app: 'settings', section: 'appearance' } },
  { label: 'GPU', icon: 'gpu', hint: 'Settings', request: { app: 'settings', section: 'gpu' } },
];
const PINNED = ENTRIES.slice(0, 8);

export function LauncherPanel({ open, onOpen, onClose }: Props) {
  const { state } = useSystem();
  const [query, setQuery] = useState('');
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery('');
      requestAnimationFrame(() => input.current?.focus());
    }
  }, [open]);

  const q = query.trim().toLowerCase();
  const results = q ? ENTRIES.filter((e) => `${e.label} ${e.hint ?? ''}`.toLowerCase().includes(q)) : PINNED;

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && results[0]) onOpen(results[0].request);
    if (e.key === 'Escape') onClose();
  };

  return (
    <Presence open={open} className="popover launcher" label="Launcher" role="dialog" id="launcher">
      <div className="search">
        <Icon name="search" size={15} />
        <label htmlFor="launcher-search" className="sr-only">Search apps, settings and places</label>
        <input id="launcher-search" ref={input} value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={onKey} placeholder="Search apps, settings and places" autoComplete="off" />
      </div>
      <div className="label" style={{ marginTop: 18 }}>{q ? 'Results' : 'Pinned'}</div>
      {results.length === 0 ? (
        <EmptyState icon="search" title="Nothing matches">Try an app name, or a setting like "memory".</EmptyState>
      ) : (
        <div className="launch-grid stagger" key={q}>
          {results.map((e, i) => (
            <button key={e.label} type="button" className="launch-btn" style={{ ['--i' as string]: i }} onClick={() => onOpen(e.request)} title={e.hint}>
              <span className="launch-badge"><Icon name={e.icon} size={24} /></span>
              {e.label}
            </button>
          ))}
        </div>
      )}
      <div className="launcher-foot">
        <div className="row" style={{ gap: 10 }}>
          <span className="avatar" aria-hidden="true">{state.user[0]}</span>
          {state.user}
        </div>
        <span className="muted" style={{ fontSize: 12 }}>Everything runs in a box</span>
      </div>
    </Presence>
  );
}
