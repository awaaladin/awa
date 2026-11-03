import { useSystem } from '../state/SystemProvider';
import { fmtGB, ramUsed } from '../state/system';

/**
 * The editorial status block in the top-left corner: "AWA OS" and three live
 * lines, in the style of the brand boards ("SMARTER / FASTER / YOURS").
 * Every number comes from the shared system state.
 */
export function SystemWidget({ onMemory }: { onMemory: () => void }) {
  const { state } = useSystem();
  const running = state.boxes.filter((b) => b.state === 'running').length;
  const jobs = state.gpu.jobs.length;
  return (
    <aside className="brand" aria-label="System status">
      <div className="brand-title">AWA OS</div>
      <button type="button" className="brand-lines" onClick={onMemory} aria-label={`${fmtGB(ramUsed(state))} of ${fmtGB(state.ramTotalGB)} in use, ${running} boxes running, ${jobs} GPU jobs. Open memory settings`}>
        <span>{fmtGB(ramUsed(state))} of {fmtGB(state.ramTotalGB)} in use</span>
        <span>{running} boxes running</span>
        <span>{jobs} GPU jobs</span>
      </button>
    </aside>
  );
}
