import { useState, type FormEvent } from 'react';
import { useSystem } from '../state/SystemProvider';
import { boxUsed, fmtGB, validBoxName, type BoxKind } from '../state/system';
import { Button } from '../ui/Button';
import { Chip } from '../ui/Chip';
import { Meter } from '../ui/Meter';
import { Segmented } from '../ui/Segmented';
import { EmptyState } from '../ui/States';
import { DownloadBrokerDialog } from './DownloadBrokerDialog';

export function BoxesApp() {
  const { state, dispatch } = useSystem();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<BoxKind>('vm');
  const [error, setError] = useState<string | null>(null);
  const [resetting, setResetting] = useState<string | null>(null);

  const pending = state.downloads.filter((d) => d.state === 'pending');
  const transfers = state.downloads.filter((d) => d.state !== 'pending');

  const create = (e: FormEvent) => {
    e.preventDefault();
    const n = name.trim();
    if (!validBoxName(n)) return setError('Use a-z, 0-9 and -, starting with a letter (max 24).');
    if (state.boxes.some((b) => b.id === n)) return setError(`A box named "${n}" already exists.`);
    dispatch({ type: 'newBox', name: n, kind });
    setCreating(false);
    setName('');
    setError(null);
  };

  const reset = (id: string) => {
    setResetting(id);
    window.setTimeout(() => {
      dispatch({ type: 'resetBox', box: id });
      setResetting(null);
    }, 500);
  };

  return (
    <div className="pane">
      <div className="row">
        <h2>Boxes</h2>
        {!creating && <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>New box</Button>}
      </div>

      {creating && (
        <form className="card" style={{ marginTop: 14 }} onSubmit={create} noValidate>
          <div className="row" style={{ alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <div style={{ flex: 1, minWidth: 200 }}>
              <label htmlFor="box-name" className="label">Name</label>
              <div className="addr" style={{ marginTop: 6, borderColor: error ? 'rgba(248,113,113,.8)' : undefined }}>
                <input id="box-name" value={name} autoFocus onChange={(e) => { setName(e.target.value); setError(null); }} placeholder="e.g. sketchy-tools" aria-invalid={!!error} aria-describedby="box-name-err" />
              </div>
            </div>
            <Segmented label="Box type" value={kind} onChange={setKind} options={[{ value: 'vm', label: 'VM box' }, { value: 'light', label: 'Light box' }]} />
          </div>
          <div id="box-name-err" className="t-error" style={{ minHeight: 18, marginTop: 6, fontSize: 12 }} aria-live="polite">{error}</div>
          <p className="muted" style={{ margin: '2px 0 0', fontSize: 12 }}>
            {kind === 'vm' ? 'Hardware-isolated microVM: for downloads and anything you do not trust. Network off.' : 'Process-level box: nearly free. For your own tools.'}
          </p>
          <div className="row" style={{ marginTop: 10, justifyContent: 'flex-end' }}>
            <Button variant="ghost" onClick={() => { setCreating(false); setError(null); }}>Cancel</Button>
            <Button variant="primary" type="submit">Create</Button>
          </div>
        </form>
      )}

      {pending.map((d) => (
        <div key={d.id} style={{ marginTop: 16 }}><DownloadBrokerDialog download={d} /></div>
      ))}

      <div className="stack stagger" style={{ marginTop: 16, gap: 8 }}>
        {state.boxes.map((b, i) => (
          <div key={b.id} className="box-row" style={{ ['--i' as string]: i }}>
            <span className={`dot is-${b.state}`} aria-hidden="true" />
            <span style={{ flex: 1, fontWeight: 600 }}>{b.id}</span>
            <Chip tone="muted">{b.kind === 'vm' ? 'VM' : 'light'}</Chip>
            <span className="muted" style={{ width: 70 }}>{b.state === 'starting' ? 'Starting…' : b.state === 'running' ? 'Running' : 'Stopped'}</span>
            <span className="mono" style={{ fontSize: 12, width: 110, textAlign: 'right' }}>{fmtGB(boxUsed(b))} / {fmtGB(b.limitGB)}</span>
            <Button size="sm" variant="ghost" icon="reset" disabled={b.id === 'system' || b.state === 'starting'} loading={resetting === b.id} onClick={() => reset(b.id)} aria-label={`Reset ${b.id}`}>
              Reset
            </Button>
          </div>
        ))}
      </div>

      <div className="label" style={{ marginTop: 20 }}>Downloads</div>
      {transfers.length === 0 ? (
        <EmptyState icon="download" title="No downloads yet">
          Try <span className="mono acc-text">get https://example.org/model.bin --into train</span> in Terminal.
        </EmptyState>
      ) : (
        <div className="stack" style={{ marginTop: 8, gap: 8 }}>
          {transfers.map((d) => (
            <div key={d.id} className="dl-row">
              <div>
                <div style={{ fontWeight: 600 }}>{d.file}</div>
                <div className="muted" style={{ fontSize: 12 }}>
                  {d.state === 'canceled' ? 'Canceled' : `into box "${d.box}" · ${fmtGB(d.received)} of ${fmtGB(d.sizeGB)}`}
                  {d.state === 'downloading' && ` · ${d.connections} connections`}
                </div>
              </div>
              <Chip tone={d.state === 'done' ? 'accent' : 'muted'}>{d.state === 'done' ? 'Done' : d.state === 'canceled' ? 'Canceled' : 'Downloading'}</Chip>
              {d.state !== 'canceled' && (
                <div style={{ gridColumn: '1 / -1' }}>
                  <Meter label={`${d.file} progress`} value={d.received} max={d.sizeGB} />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
