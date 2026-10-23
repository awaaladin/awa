import { Presence } from '../motion/presence';
import { useSystem } from '../state/SystemProvider';
import { useTheme } from '../theme/ThemeProvider';
import { FEEL_IDS, SURFACE_IDS, tokens } from '../tokens';
import { Segmented } from '../ui/Segmented';
import { Slider } from '../ui/Slider';
import { Tile } from '../ui/Tile';

type Props = { open: boolean };

const pct = (v: number) => `${Math.round(v * 100)}%`;

export function QuickSettings({ open }: Props) {
  const { state, dispatch } = useSystem();
  const theme = useTheme();
  const q = state.quick;
  const set = (key: keyof typeof q, value: boolean | number) => dispatch({ type: 'quick', key, value });

  return (
    <Presence open={open} className="popover quick" label="Quick settings" role="dialog" id="quick-settings" from={{ scale: 0.97, y: 10 }}>
      <div className="quick-grid">
        <Tile title="Wi-Fi" detail={q.wifi ? 'Home network' : 'Off'} on={q.wifi} icon="wifi" onClick={() => set('wifi', !q.wifi)} />
        <Tile title="Bluetooth" detail={q.bluetooth ? 'On' : 'Off'} on={q.bluetooth} icon="bluetooth" onClick={() => set('bluetooth', !q.bluetooth)} />
        <Tile title="Box isolation" detail={q.strictIsolation ? 'Strict' : 'Relaxed'} on={q.strictIsolation} icon="shield" onClick={() => set('strictIsolation', !q.strictIsolation)} />
        <Tile title="Low memory" detail={theme.lowMemory ? 'On: no blur, less motion' : 'Off'} on={theme.lowMemory} icon="feather" onClick={() => theme.setLowMemory(!theme.lowMemory)} />
      </div>

      <div className="label" style={{ marginTop: 14 }}>Surface</div>
      <div style={{ marginTop: 8 }}>
        <Segmented
          label="Surface"
          value={theme.surface}
          onChange={theme.setSurface}
          options={SURFACE_IDS.map((s) => ({ value: s, label: tokens.surfaces[s].label }))}
        />
      </div>

      <div className="label" style={{ marginTop: 14 }}>Feel</div>
      <div className="swatches" style={{ marginTop: 8 }} role="group" aria-label="Feel">
        {FEEL_IDS.map((id) => (
          <button
            key={id}
            type="button"
            className="swatch"
            aria-label={`Use the ${tokens.feels[id].name} feel`}
            aria-pressed={theme.feel === id}
            style={{ background: tokens.feels[id].accent }}
            onClick={() => theme.setFeel(id)}
          />
        ))}
      </div>

      <div className="stack" style={{ marginTop: 14 }}>
        <Slider label="Volume" value={q.volume} min={0} max={1} step={0.01} format={pct} onChange={(v) => set('volume', v)} />
        <Slider label="Brightness" value={q.brightness} min={0.1} max={1} step={0.01} format={pct} onChange={(v) => set('brightness', v)} />
      </div>
    </Presence>
  );
}
