import { useState, type FormEvent } from 'react';
import { useSystem } from '../state/SystemProvider';
import { logoIcon } from '../tokens';
import { Button } from '../ui/Button';
import { Chip } from '../ui/Chip';
import { Icon } from '../ui/Icon';
import { ErrorNote } from '../ui/States';

type Page =
  | { kind: 'home' }
  | { kind: 'loading'; host: string }
  | { kind: 'site'; host: string }
  | { kind: 'blocked'; host: string };

/** Hosts this prototype "has" pages for. Anything else shows the network-blocked state. */
const KNOWN = ['docs.rs', 'arxiv.org', 'huggingface.co'];

const hostOf = (text: string) => {
  try {
    return new URL(text.includes('://') ? text : `https://${text}`).hostname;
  } catch {
    return null;
  }
};

/** A boxed browser tab. Downloads go to the broker, never to the home folder. */
export function BrowserApp({ onOpenBroker }: { onOpenBroker: () => void }) {
  const { dispatch } = useSystem();
  const [page, setPage] = useState<Page>({ kind: 'home' });
  const [address, setAddress] = useState('');
  const [allowed, setAllowed] = useState<string[]>([]);

  const go = (target: string) => {
    const host = hostOf(target.trim());
    if (!host) return;
    setAddress(host);
    setPage({ kind: 'loading', host });
    window.setTimeout(() => {
      setPage(KNOWN.includes(host) || allowed.includes(host) ? { kind: 'site', host } : { kind: 'blocked', host });
    }, 550);
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    go(address);
  };

  const download = (url: string) => {
    dispatch({ type: 'startDownload', url, box: null });
    onOpenBroker();
  };

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
      <form className="row" style={{ padding: '10px 14px', borderBottom: '1px solid var(--line)' }} onSubmit={submit}>
        <button type="button" className="icon-btn" aria-label="Home" onClick={() => { setPage({ kind: 'home' }); setAddress(''); }}>
          <Icon name="home" size={16} />
        </button>
        <div className="addr">
          <Icon name="shield" size={14} stroke="var(--acc-text)" strokeWidth={2} />
          <label htmlFor="addr" className="sr-only">Address</label>
          <input id="addr" value={address} onChange={(e) => setAddress(e.target.value)} placeholder="Search or type an address" autoComplete="off" />
          {page.kind === 'loading' && <span className="spinner" aria-label="Loading" />}
        </div>
        <Chip tone="accent">Boxed tab</Chip>
      </form>

      <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column' }}>
        {page.kind === 'home' && (
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 16, padding: 24 }}>
            <img src={logoIcon} alt="AWA" width={72} height={78} />
            <h2 style={{ fontSize: 26 }}>Every tab lives in its own box</h2>
            <p className="muted" style={{ margin: 0 }}>Downloads stop at the broker until you say where they go.</p>
            <div className="row stagger" style={{ gap: 12, marginTop: 6 }}>
              {KNOWN.map((h, i) => (
                <button key={h} type="button" className="link-tile" style={{ ['--i' as string]: i }} onClick={() => go(h)}>{h}</button>
              ))}
            </div>
          </div>
        )}

        {page.kind === 'loading' && (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10 }} className="muted">
            <span className="spinner" aria-hidden="true" /> Opening {page.host} in a fresh box…
          </div>
        )}

        {page.kind === 'blocked' && (
          <div className="pane">
            <ErrorNote
              title={`${page.host} is not allowed in this box`}
              action={<Button size="sm" variant="secondary" onClick={() => { setAllowed((a) => [...a, page.host]); go(page.host); }}>Allow {page.host} for this tab</Button>}
            >
              Box isolation is strict: tabs only reach sites you allow. Nothing was sent.
            </ErrorNote>
          </div>
        )}

        {page.kind === 'site' && (
          <div className="pane">
            <div className="row" style={{ justifyContent: 'flex-start', gap: 10 }}>
              <Icon name="browser" size={18} stroke="var(--acc-text)" />
              <h2>{page.host}</h2>
            </div>
            {page.host === 'huggingface.co' ? (
              <div className="card" style={{ marginTop: 16 }}>
                <div style={{ fontWeight: 600 }}>awa-demo / pose-v3</div>
                <p className="muted" style={{ margin: '4px 0 12px' }}>Pose-estimation weights, 1.2 GB.</p>
                <Button variant="primary" icon="download" onClick={() => download('https://huggingface.co/awa-demo/pose-v3/resolve/main/model-weights.bin')}>
                  Download model-weights.bin
                </Button>
              </div>
            ) : (
              <p className="muted" style={{ marginTop: 12 }}>This is a stand-in page. The tab runs in its own disposable box and keeps nothing after you close it.</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
