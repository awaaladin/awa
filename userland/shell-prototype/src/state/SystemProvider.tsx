import { createContext, useContext, useEffect, useReducer, type Dispatch, type ReactNode } from 'react';
import { initialSystem, reduce, type Action, type SystemState } from './system';

const Ctx = createContext<{ state: SystemState; dispatch: Dispatch<Action> } | null>(null);

export function SystemProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reduce, initialSystem);

  // Downloads advance only while something is downloading.
  const active = state.downloads.some((d) => d.state === 'downloading');
  useEffect(() => {
    if (!active) return;
    const id = window.setInterval(() => dispatch({ type: 'tick', seconds: 0.25 }), 250);
    return () => window.clearInterval(id);
  }, [active]);

  // New boxes take a moment to start (a microVM boots; a light box is instant-ish).
  const starting = state.boxes.filter((b) => b.state === 'starting');
  useEffect(() => {
    const timers = starting.map((b) => window.setTimeout(() => dispatch({ type: 'boxStarted', box: b.id }), b.kind === 'vm' ? 1400 : 300));
    return () => timers.forEach((t) => window.clearTimeout(t));
  }, [starting.map((b) => b.id).join()]);

  return <Ctx.Provider value={{ state, dispatch }}>{children}</Ctx.Provider>;
}

export function useSystem() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useSystem outside SystemProvider');
  return v;
}
