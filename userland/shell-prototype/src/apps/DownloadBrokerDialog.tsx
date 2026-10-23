import { useEffect, useRef } from 'react';
import { useSystem } from '../state/SystemProvider';
import { fmtGB, type Download } from '../state/system';
import { Button } from '../ui/Button';

type Props = { download: Download };

/**
 * The download broker's question. Nothing is fetched until the owner picks a
 * box; the fetch then runs inside that box (spec 07 §5).
 */
export function DownloadBrokerDialog({ download }: Props) {
  const { state, dispatch } = useSystem();
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus({ preventScroll: true }), [download.id]);

  const candidates = state.boxes.filter((b) => b.id !== 'system');
  const suggested = /weights|model|\.bin$|\.pt$/i.test(download.file) ? candidates.find((b) => b.id === 'train') : undefined;
  const others = candidates.filter((b) => b !== suggested);

  const throwaway = () => {
    let n = 1;
    while (state.boxes.some((b) => b.id === `scan-${n}`)) n++;
    const name = `scan-${n}`;
    dispatch({ type: 'newBox', name, kind: 'vm' });
    dispatch({ type: 'routeDownload', id: download.id, box: name });
  };

  return (
    <section className="broker" role="alertdialog" aria-labelledby={`broker-${download.id}`} aria-describedby={`broker-d-${download.id}`}>
      <h3 id={`broker-${download.id}`}>Where should this download go?</h3>
      <p className="muted" id={`broker-d-${download.id}`} style={{ margin: '4px 0 0' }}>
        {download.file} · {fmtGB(download.sizeGB)} · from {download.from}
      </p>
      <div className="row" style={{ marginTop: 14, justifyContent: 'flex-start', flexWrap: 'wrap', gap: 10 }}>
        {suggested && (
          <Button ref={first} variant="primary" onClick={() => dispatch({ type: 'routeDownload', id: download.id, box: suggested.id })}>
            Into “{suggested.id}” box
          </Button>
        )}
        <Button ref={suggested ? undefined : first} variant={suggested ? 'secondary' : 'primary'} icon="shield" onClick={throwaway}>
          Scan in a throwaway box
        </Button>
        {others.slice(0, 2).map((b) => (
          <Button key={b.id} variant="secondary" onClick={() => dispatch({ type: 'routeDownload', id: download.id, box: b.id })}>
            Into “{b.id}”
          </Button>
        ))}
        <Button variant="ghost" onClick={() => dispatch({ type: 'cancelDownload', id: download.id })}>Cancel</Button>
      </div>
    </section>
  );
}
