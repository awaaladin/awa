import { fmtShortDate, fmtTime, useClock } from '../motion/useClock';
import { useSystem } from '../state/SystemProvider';
import { fmtGB, ramUsed, vramUsed } from '../state/system';
import { Meter } from '../ui/Meter';

/** Desktop clock with live RAM and GPU, from the same data as everything else. */
export function SystemWidget() {
  const { state } = useSystem();
  const now = useClock();
  const ram = ramUsed(state);
  const vram = vramUsed(state);
  return (
    <aside className="widget glass" aria-label="System status">
      <div className="widget-time">
        <b>{fmtTime(now)}</b>
        <span className="muted" style={{ fontSize: 12 }}>{fmtShortDate(now)}</span>
      </div>
      <span className="label">Memory · measured by AWA</span>
      <Meter label="Memory in use" value={ram} max={state.ramTotalGB} detail={<><span>{fmtGB(ram)}</span><span className="muted">of {fmtGB(state.ramTotalGB)}</span></>} />
      <span className="label">GPU · {state.gpu.jobs.length} jobs queued</span>
      <Meter label="GPU memory in use" tone="acc2" value={vram} max={state.gpu.vramTotalGB} detail={<span>{vram.toFixed(1)} / {state.gpu.vramTotalGB} GB VRAM</span>} />
    </aside>
  );
}
