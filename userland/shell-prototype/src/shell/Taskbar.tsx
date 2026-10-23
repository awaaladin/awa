import { fmtTime, useClock } from '../motion/useClock';
import { useSystem } from '../state/SystemProvider';
import { ramUsed } from '../state/system';
import { logoIcon } from '../tokens';
import { Icon } from '../ui/Icon';
import { APP_IDS, APPS, type AppId } from './apps';

type Props = {
  open: Set<AppId>;
  focused: AppId | null;
  launcherOpen: boolean;
  quickOpen: boolean;
  onLauncher: () => void;
  onQuick: () => void;
  onApp: (id: AppId) => void;
  onMemory: () => void;
};

export function Taskbar({ open, focused, launcherOpen, quickOpen, onLauncher, onQuick, onApp, onMemory }: Props) {
  const { state } = useSystem();
  const now = useClock();
  return (
    <nav className="taskbar glass" aria-label="Taskbar">
      <div className="taskbar-left">
        <button
          type="button"
          className={`dock-btn ${launcherOpen ? 'is-focused' : ''}`}
          aria-label="Open launcher"
          aria-expanded={launcherOpen}
          aria-controls="launcher"
          onClick={onLauncher}
        >
          <img src={logoIcon} alt="" className="dock-logo" />
        </button>
        <div className="taskbar-sep" aria-hidden="true" />
        {APP_IDS.map((id) => (
          <button
            key={id}
            type="button"
            className={`dock-btn ${open.has(id) ? 'is-open' : ''} ${focused === id ? 'is-focused' : ''}`}
            aria-label={`${APPS[id].title}${open.has(id) ? ' (open)' : ''}`}
            aria-pressed={focused === id}
            onClick={() => onApp(id)}
          >
            <Icon name={APPS[id].icon} size={20} />
          </button>
        ))}
      </div>
      <div className="taskbar-right">
        <button type="button" className="ram-pill" onClick={onMemory} aria-label={`Memory: ${ramUsed(state).toFixed(1)} of ${state.ramTotalGB} GB used. Open memory settings`}>
          RAM {ramUsed(state).toFixed(1)} / {state.ramTotalGB} GB
        </button>
        <button
          type="button"
          className={`tray ${quickOpen ? 'is-active' : ''}`}
          aria-label="Quick settings"
          aria-expanded={quickOpen}
          aria-controls="quick-settings"
          onClick={onQuick}
        >
          <Icon name="wifi" size={17} stroke={state.quick.wifi ? 'currentColor' : 'var(--muted)'} />
          <Icon name="battery" size={19} />
          <b>{fmtTime(now)}</b>
        </button>
      </div>
    </nav>
  );
}
