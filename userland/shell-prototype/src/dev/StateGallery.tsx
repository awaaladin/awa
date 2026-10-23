import { useState, type ReactNode } from 'react';
import { useTheme } from '../theme/ThemeProvider';
import { FEEL_IDS, SURFACE_IDS, tokens } from '../tokens';
import { Button } from '../ui/Button';
import { Chip } from '../ui/Chip';
import { IconButton } from '../ui/IconButton';
import { Meter } from '../ui/Meter';
import { Segmented } from '../ui/Segmented';
import { Slider } from '../ui/Slider';
import { EmptyState, ErrorNote } from '../ui/States';
import { Tile } from '../ui/Tile';
import { Toggle } from '../ui/Toggle';

/**
 * Every shared control in every state it has. Hover, focus-visible and
 * pressed are live: hover, Tab or click to see them. ?screen=gallery
 */
export function StateGallery() {
  const t = useTheme();
  const [on, setOn] = useState(true);
  const [v, setV] = useState(0.4);
  return (
    <main style={{ height: '100%', overflowY: 'auto', background: 'var(--win)', padding: 32 }}>
      <div className="row" style={{ flexWrap: 'wrap' }}>
        <h1 style={{ fontSize: 28 }}>AWA controls · states</h1>
        <div className="row" style={{ gap: 10 }}>
          <Segmented label="Surface" value={t.surface} onChange={t.setSurface} options={SURFACE_IDS.map((s) => ({ value: s, label: tokens.surfaces[s].label }))} />
          <Segmented label="Feel" value={t.feel} onChange={t.setFeel} options={FEEL_IDS.slice(0, 8).map((f) => ({ value: f, label: tokens.feels[f].name }))} />
        </div>
      </div>
      <p className="muted">Hover, focus-visible (Tab) and pressed states are live on every control below.</p>

      <Row title="Button">
        <Button variant="primary">Primary</Button>
        <Button>Secondary</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="danger">Danger</Button>
        <Button variant="primary" disabled>Disabled</Button>
        <Button variant="primary" loading>Loading</Button>
        <Button icon="download" size="sm">Small with icon</Button>
      </Row>
      <Row title="IconButton">
        <IconButton icon="settings" label="Default" />
        <IconButton icon="settings" label="Active" active />
        <IconButton icon="settings" label="Disabled" disabled />
      </Row>
      <Row title="Tile">
        <div style={{ width: 160 }}><Tile title="Wi-Fi" detail="Home network" on icon="wifi" /></div>
        <div style={{ width: 160 }}><Tile title="Bluetooth" detail="Off" icon="bluetooth" /></div>
        <div style={{ width: 160 }}><Tile title="Unavailable" detail="No adapter" icon="bluetooth" disabled /></div>
      </Row>
      <Row title="Chip">
        <Chip>Default</Chip><Chip tone="accent">Recomputable</Chip><Chip tone="muted">VM box</Chip><Chip tone="warn">Over limit</Chip>
      </Row>
      <Row title="Meter">
        <div style={{ width: 220 }}><Meter label="Empty" value={0} max={8} detail={<span>0 GB</span>} /></div>
        <div style={{ width: 220 }}><Meter label="Partial" value={1.9} max={8} detail={<span>1.9 / 8 GB</span>} /></div>
        <div style={{ width: 220 }}><Meter label="Full" tone="acc2" value={8} max={8} detail={<span>8 / 8 GB</span>} /></div>
      </Row>
      <Row title="Slider">
        <div style={{ width: 320 }}><Slider label="Live" value={v} min={0} max={1} step={0.01} onChange={setV} format={(x) => `${Math.round(x * 100)}%`} /></div>
        <div style={{ width: 320 }}><Slider label="Disabled" value={0.6} min={0} max={1} step={0.01} onChange={() => {}} disabled format={(x) => `${Math.round(x * 100)}%`} /></div>
      </Row>
      <Row title="Toggle">
        <div style={{ width: 320 }}><Toggle label="Live toggle" checked={on} onChange={setOn} description="Click or press Space." /></div>
        <div style={{ width: 320 }}><Toggle label="Disabled" checked={false} onChange={() => {}} disabled /></div>
      </Row>
      <Row title="Empty and error">
        <div className="card" style={{ width: 340 }}><EmptyState icon="download" title="No downloads yet">Downloads appear here once a box receives them.</EmptyState></div>
        <div style={{ width: 380 }}><ErrorNote title="example.com is not allowed in this box" action={<Button size="sm">Allow for this tab</Button>}>Nothing was sent.</ErrorNote></div>
      </Row>
    </main>
  );
}

function Row({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section style={{ marginTop: 24 }}>
      <div className="label">{title}</div>
      <div className="row" style={{ justifyContent: 'flex-start', flexWrap: 'wrap', gap: 12, marginTop: 10 }}>{children}</div>
    </section>
  );
}
