import { useCallback, useEffect, useRef, useState } from 'react';
import { BoxesApp } from '../apps/BoxesApp';
import { BrowserApp } from '../apps/BrowserApp';
import { FilesApp } from '../apps/FilesApp';
import { SettingsApp } from '../apps/SettingsApp';
import { TerminalApp } from '../apps/TerminalApp';
import { useSystem } from '../state/SystemProvider';
import { Icon } from '../ui/Icon';
import { APPS, type AppId, type OpenRequest, type SettingsSection } from './apps';
import { LauncherPanel } from './LauncherPanel';
import { QuickSettings } from './QuickSettings';
import { SystemWidget } from './SystemWidget';
import { Taskbar } from './Taskbar';
import { Wallpaper } from './Wallpaper';
import { WindowFrame, type Rect } from './WindowFrame';

type Win = { id: AppId; rect: Rect; z: number; minimized: boolean; maximized: boolean; closing: boolean; section?: SettingsSection };

const BASE: Rect = { x: 210, y: 56, w: 800, h: 540 };

function placeFor(count: number): Rect {
  const w = Math.min(BASE.w, window.innerWidth - 40);
  const h = Math.min(BASE.h, window.innerHeight - 120);
  const step = 28 * (count % 6);
  return { x: Math.min(BASE.x + step, window.innerWidth - w - 20), y: Math.min(BASE.y + step, window.innerHeight - h - 80), w, h };
}

type Props = { initial?: OpenRequest[] };

export function Desktop({ initial = [{ app: 'settings', section: 'appearance' }] }: Props) {
  const { state } = useSystem();
  const z = useRef(10);
  const [wins, setWins] = useState<Win[]>(() =>
    initial.map((r, i) => ({ id: r.app, rect: placeFor(i), z: ++z.current, minimized: false, maximized: false, closing: false, section: r.section })),
  );
  const [focused, setFocused] = useState<AppId | null>(initial.at(-1)?.app ?? null);
  const [launcher, setLauncher] = useState(false);
  const [quick, setQuick] = useState(false);

  const update = (id: AppId, patch: Partial<Win>) => setWins((ws) => ws.map((w) => (w.id === id ? { ...w, ...patch } : w)));

  const open = useCallback((r: OpenRequest) => {
    setLauncher(false);
    setQuick(false);
    setWins((ws) => {
      const top = ++z.current;
      const found = ws.find((w) => w.id === r.app);
      if (found) return ws.map((w) => (w.id === r.app ? { ...w, z: top, minimized: false, closing: false, section: r.section ?? w.section } : w));
      return [...ws, { id: r.app, rect: placeFor(ws.length), z: top, minimized: false, maximized: false, closing: false, section: r.section }];
    });
    setFocused(r.app);
  }, []);

  const focus = (id: AppId) => {
    if (focused === id) return;
    update(id, { z: ++z.current });
    setFocused(id);
  };

  const topmostOther = (id: AppId) =>
    wins.filter((w) => w.id !== id && !w.minimized && !w.closing).sort((a, b) => b.z - a.z)[0]?.id ?? null;

  const minimize = (id: AppId) => {
    update(id, { minimized: true });
    setFocused(topmostOther(id));
  };

  const onTaskbarApp = (id: AppId) => {
    const w = wins.find((x) => x.id === id);
    if (w && focused === id && !w.minimized && !w.closing) minimize(id);
    else open({ app: id });
  };

  // When something asks the broker a question, bring Boxes forward so it is answered.
  const pending = state.downloads.filter((d) => d.state === 'pending').length;
  const lastPending = useRef(pending);
  useEffect(() => {
    if (pending > lastPending.current) open({ app: 'boxes' });
    lastPending.current = pending;
  }, [pending, open]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setLauncher(false);
        setQuick(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const openIds = new Set(wins.filter((w) => !w.closing).map((w) => w.id));

  return (
    <main className="desktop" onPointerDown={(e) => {
      if (e.target === e.currentTarget || (e.target as HTMLElement).classList.contains('wall-layer')) {
        setLauncher(false);
        setQuick(false);
      }
    }}>
      <Wallpaper />

      <div className="desk-icons">
        {([['files', 'home', 'Home'], ['boxes', 'box', 'Boxes'], ['terminal', 'terminal', 'Terminal']] as const).map(([app, icon, label]) => (
          <button key={app} type="button" className="desk-icon" onClick={() => open({ app })}>
            <span className="desk-icon-badge glass"><Icon name={icon} size={24} /></span>
            {label}
          </button>
        ))}
      </div>

      <SystemWidget />

      {wins.map((w) => (
        <WindowFrame
          key={w.id}
          title={APPS[w.id].title}
          rect={w.rect}
          z={w.z}
          focused={focused === w.id}
          maximized={w.maximized}
          minimized={w.minimized}
          closing={w.closing}
          onFocus={() => focus(w.id)}
          onMove={(x, y) => update(w.id, { rect: { ...w.rect, x, y } })}
          onMinimize={() => minimize(w.id)}
          onToggleMax={() => update(w.id, { maximized: !w.maximized })}
          onClose={() => {
            update(w.id, { closing: true });
            if (focused === w.id) setFocused(topmostOther(w.id));
          }}
          onClosed={() => setWins((ws) => ws.filter((x) => !(x.id === w.id && x.closing)))}
        >
          {w.id === 'settings' && <SettingsApp section={w.section} />}
          {w.id === 'files' && <FilesApp onOpenBoxes={() => open({ app: 'boxes' })} />}
          {w.id === 'terminal' && <TerminalApp focused={focused === 'terminal'} onOpenBroker={() => open({ app: 'boxes' })} />}
          {w.id === 'browser' && <BrowserApp onOpenBroker={() => open({ app: 'boxes' })} />}
          {w.id === 'boxes' && <BoxesApp />}
        </WindowFrame>
      ))}

      <LauncherPanel open={launcher} onOpen={open} onClose={() => setLauncher(false)} />
      <QuickSettings open={quick} />

      <Taskbar
        open={openIds}
        focused={focused}
        launcherOpen={launcher}
        quickOpen={quick}
        onLauncher={() => { setLauncher((v) => !v); setQuick(false); }}
        onQuick={() => { setQuick((v) => !v); setLauncher(false); }}
        onApp={onTaskbarApp}
        onMemory={() => open({ app: 'settings', section: 'memory' })}
      />
    </main>
  );
}
