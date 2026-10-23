import { useEffect, useState } from 'react';
import { useSystem } from '../state/SystemProvider';
import { boxClass, boxUsed, declared, fmtGB, ramUsed, round, tradeoff, vramUsed, type Box } from '../state/system';
import { useTheme } from '../theme/ThemeProvider';
import { FEEL_IDS, SURFACE_IDS, tokens, WALLPAPERS } from '../tokens';
import type { SettingsSection } from '../shell/apps';
import { Button } from '../ui/Button';
import { Chip } from '../ui/Chip';
import { Icon, type IconName } from '../ui/Icon';
import { Meter } from '../ui/Meter';
import { Slider } from '../ui/Slider';
import { Toggle } from '../ui/Toggle';

const SECTIONS: { id: SettingsSection; label: string; icon: IconName }[] = [
  { id: 'appearance', label: 'Appearance', icon: 'sun' },
  { id: 'memory', label: 'Memory', icon: 'memory' },
  { id: 'gpu', label: 'GPU', icon: 'gpu' },
];

export function SettingsApp({ section: requested }: { section?: SettingsSection }) {
  const [section, setSection] = useState<SettingsSection>(requested ?? 'appearance');
  useEffect(() => {
    if (requested) setSection(requested);
  }, [requested]);

  return (
    <>
      <nav className="side" aria-label="Settings sections">
        {SECTIONS.map((s) => (
          <button key={s.id} type="button" className="side-btn" aria-current={section === s.id ? 'page' : undefined} onClick={() => setSection(s.id)}>
            <Icon name={s.icon} size={16} />
            {s.label}
          </button>
        ))}
      </nav>
      <div className="pane">
        {section === 'appearance' && <Appearance />}
        {section === 'memory' && <Memory />}
        {section === 'gpu' && <Gpu />}
      </div>
    </>
  );
}

function Appearance() {
  const t = useTheme();
  return (
    <>
      <h2>Appearance</h2>
      <p className="lede">Pick a feel. The wallpaper, accent and glass change everywhere.</p>

      <div className="label" style={{ marginTop: 18 }}>Surface</div>
      <div className="surface-grid" role="radiogroup" aria-label="Surface" style={{ marginTop: 8 }}>
        {SURFACE_IDS.map((id) => {
          const s = tokens.surfaces[id];
          const win = s.window === 'feel' ? tokens.feels[t.feel].window : s.window;
          return (
            <button key={id} type="button" role="radio" aria-checked={t.surface === id} className="surface-card" onClick={() => t.setSurface(id)}>
              <div className="surface-preview" style={{ background: win }}>
                <span style={{ width: 34, background: tokens.feels[t.feel].accent }} />
                <span style={{ width: 22, background: s.text, opacity: 0.6 }} />
              </div>
              <div className="feel-name">{s.label}</div>
              <div className="feel-note">{s.note}</div>
            </button>
          );
        })}
      </div>

      <div className="label" style={{ marginTop: 18 }}>Feel</div>
      <div className="feel-grid stagger" role="radiogroup" aria-label="Feel" style={{ marginTop: 8 }}>
        {FEEL_IDS.map((id, i) => {
          const f = tokens.feels[id];
          return (
            <button key={id} type="button" role="radio" aria-checked={t.feel === id} aria-label={`${f.name}: ${f.note}`} className="feel-card" style={{ ['--i' as string]: i }} onClick={() => t.setFeel(id)}>
              <img src={WALLPAPERS[id]} alt="" loading="lazy" />
              <div className="feel-name">{f.name}</div>
              <div className="feel-note">{f.note}</div>
            </button>
          );
        })}
      </div>

      <div className="card" style={{ marginTop: 18 }}>
        <Toggle
          label="Low memory mode"
          checked={t.lowMemory}
          onChange={t.setLowMemory}
          description="Turns off glass blur, the wallpaper fade and heavy animation, and boots with the smaller 480p video. Saves memory and GPU time on small machines."
        />
      </div>
      <p className="muted" style={{ marginTop: 10, fontSize: 12 }}>
        Motion: {t.reducedMotion ? 'reduced, following your system setting' : 'full'}. AWA also follows the system "reduce motion" setting.
      </p>
    </>
  );
}

function Memory() {
  const { state } = useSystem();
  const used = ramUsed(state);
  const d = declared(state);
  return (
    <>
      <h2>Memory</h2>
      <p className="lede">
        Every box has a limit you can change. Run <span className="mono acc-text">ram set render 2GB</span> for the same thing.
      </p>
      <div style={{ marginTop: 14 }}>
        <Meter label="Memory in use on this machine" value={used} max={state.ramTotalGB} detail={<><span>{fmtGB(used)} used</span><span className="muted">{fmtGB(state.ramTotalGB)} installed</span></>} />
      </div>
      <div className="row" style={{ marginTop: 14, alignItems: 'stretch' }}>
        <div className="card" style={{ flex: 1 }}>
          <div className="label">Measured by AWA</div>
          <div className="big-num">{fmtGB(used)}</div>
          <div className="muted" style={{ fontSize: 12 }}>What the kernel counts, for every app.</div>
        </div>
        <div className="card-dashed" style={{ flex: 1 }}>
          <div className="label">Declared by apps</div>
          <div className="big-num">{fmtGB(d.gb)}</div>
          <div className="muted" style={{ fontSize: 12 }}>{d.reporting} of {d.total} apps report their own use.</div>
        </div>
      </div>
      <div className="stack" style={{ marginTop: 14 }}>
        {state.boxes.map((b) => <BoxLimit key={b.id} box={b} />)}
      </div>
    </>
  );
}

/** One box: usage, a limit slider, and the cost of the new limit before it is applied. */
function BoxLimit({ box }: { box: Box }) {
  const { state, dispatch } = useSystem();
  const [draft, setDraft] = useState(box.limitGB);
  const [applying, setApplying] = useState(false);
  useEffect(() => setDraft(box.limitGB), [box.limitGB]);

  const used = boxUsed(box);
  const changed = round(draft) !== round(box.limitGB);
  const report = tradeoff(box, draft);

  const apply = () => {
    setApplying(true);
    // Reclaim is not instant in the real system; the prototype shows that with a short wait.
    window.setTimeout(() => {
      dispatch({ type: 'setLimit', box: box.id, gb: draft });
      setApplying(false);
    }, report.reclaimGB > 0 ? 700 : 250);
  };

  return (
    <div className="card">
      <div className="row">
        <span style={{ fontWeight: 600 }}>{box.id}</span>
        <span className="row" style={{ gap: 6 }}>
          {box.kind === 'vm' && <Chip tone="muted">VM box</Chip>}
          {box.state !== 'running' && <Chip tone="muted">{box.state}</Chip>}
          <Chip tone="accent">{boxClass(box)}</Chip>
        </span>
      </div>
      <div style={{ marginTop: 8 }}>
        <Meter label={`${box.id} memory`} value={used} max={box.limitGB} detail={<><span>{fmtGB(used)} used</span><span className="muted">limit {fmtGB(box.limitGB)}</span></>} />
      </div>
      <div style={{ marginTop: 8 }}>
        <Slider label={`Limit for ${box.id}`} shortLabel="Limit" value={draft} min={0.25} max={state.ramTotalGB} step={0.25} onChange={setDraft} format={fmtGB} disabled={applying || box.id === 'system'} />
      </div>
      {box.id === 'system' && <p className="muted" style={{ margin: '6px 0 0', fontSize: 12 }}>The system box limit is fixed in this prototype.</p>}
      {changed && (
        <>
          <div className="report" aria-live="polite">
            {report.lines.map((l, i) => (
              <div key={i} className="report-line">
                <Chip tone={l.source === 'measured' ? 'muted' : 'accent'}>{l.source === 'measured' ? 'measured' : 'declared'}</Chip>
                <span>{l.text}</span>
              </div>
            ))}
          </div>
          <div className="row" style={{ marginTop: 10, justifyContent: 'flex-end' }}>
            <Button variant="ghost" size="sm" onClick={() => setDraft(box.limitGB)} disabled={applying}>Cancel</Button>
            <Button variant="primary" size="sm" onClick={apply} loading={applying} disabled={!report.ok}>
              {applying ? 'Applying' : `Set to ${fmtGB(draft)}`}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

function Gpu() {
  const { state } = useSystem();
  const used = vramUsed(state);
  return (
    <>
      <h2>GPU</h2>
      <p className="lede">The GPU is scheduled like the CPU. Nothing waits behind a render you did not start.</p>
      <div className="card" style={{ marginTop: 16, padding: 16 }}>
        <div className="row" style={{ fontWeight: 600 }}>
          <span>{state.gpu.name}</span>
          <span className="mono acc-text" style={{ fontWeight: 400 }}>{used.toFixed(1)} / {state.gpu.vramTotalGB} GB VRAM</span>
        </div>
        <div style={{ marginTop: 10 }}>
          <Meter label="GPU memory" value={used} max={state.gpu.vramTotalGB} detail={<><span>display {fmtGB(state.gpu.displayGB)}</span><span className="muted">jobs {fmtGB(round(used - state.gpu.displayGB))}</span></>} />
        </div>
      </div>
      <div className="stack" style={{ marginTop: 12 }}>
        {state.gpu.jobs.map((j) => (
          <div key={j.id} className="card">
            <div className="row">
              <span>{j.label}</span>
              <span className="row" style={{ gap: 6 }}>
                <Chip tone="muted">box {j.box}</Chip>
                <Chip tone="accent">{j.sched}</Chip>
              </span>
            </div>
            <div style={{ marginTop: 8 }}>
              <Meter label={`${j.label} progress`} tone="acc2" value={j.progress} max={1} detail={<><span>{Math.round(j.progress * 100)}% done</span><span className="muted">{fmtGB(j.vramGB)} VRAM</span></>} />
            </div>
          </div>
        ))}
      </div>
    </>
  );
}
