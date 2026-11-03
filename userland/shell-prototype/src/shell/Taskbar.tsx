import { fmtTime, useClock } from '../motion/useClock';
import { useSystem } from '../state/SystemProvider';
import { logoIcon } from '../tokens';
import { APP_IDS, APPS, type AppId } from './apps';

type Props = {
  open: AppId[];
  focused: AppId | null;
  launcherOpen: boolean;
  quickOpen: boolean;
  onLauncher: () => void;
  onQuick: () => void;
  onApp: (id: AppId) => void;
  onCycle: () => void;
};

/**
 * AWA's navigation, from the brand boards rather than a Windows/macOS bar:
 * an app rail on the right (thin rule + spaced capitals), a window counter
 * in the bottom-right corner, and the mark, clock and quick settings in the
 * bottom-left corner.
 */
export function Taskbar({ open, focused, launcherOpen, quickOpen, onLauncher, onQuick, onApp, onCycle }: Props) {
  const { state } = useSystem();
  const now = useClock();
  const index = focused ? open.indexOf(focused) : -1;
  const pad = (n: number) => String(n).padStart(2, '0');

  return (
    <>
      <nav className="rail" aria-label="Apps">
        <span className="rail-line" aria-hidden="true" />
        {APP_IDS.map((id) => (
          <button
            key={id}
            type="button"
            className={`rail-btn ${open.includes(id) ? 'is-open' : ''} ${focused === id ? 'is-focused' : ''}`}
            aria-pressed={focused === id}
            aria-label={`${APPS[id].title}${open.includes(id) ? ', open' : ''}`}
            onClick={() => onApp(id)}
          >
            {APPS[id].title}
          </button>
        ))}
      </nav>

      <button
        type="button"
        className="counter"
        onClick={onCycle}
        disabled={open.length < 2}
        aria-label={open.length ? `Window ${Math.max(index, 0) + 1} of ${open.length}. Switch to the next window` : 'No windows open'}
      >
        {pad(index >= 0 ? index + 1 : 0)} <span className="counter-sep">/</span> {pad(open.length)}
      </button>

      <div className="statusline">
        <button type="button" className={`mark-btn ${launcherOpen ? 'is-active' : ''}`} aria-label="Open launcher" aria-expanded={launcherOpen} aria-controls="launcher" onClick={onLauncher}>
          <img src={logoIcon} alt="" />
        </button>
        <button type="button" className={`status-btn ${quickOpen ? 'is-active' : ''}`} aria-label="Quick settings" aria-expanded={quickOpen} aria-controls="quick-settings" onClick={onQuick}>
          <b>{fmtTime(now)}</b>
          <span>{state.quick.wifi ? 'Wi-Fi' : 'Offline'}</span>
          <span>{state.quick.strictIsolation ? 'Strict boxes' : 'Relaxed boxes'}</span>
        </button>
      </div>
    </>
  );
}
