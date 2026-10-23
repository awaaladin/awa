import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from 'react';
import { isFeelId, isSurfaceId, themeVars, tokens, type FeelId, type SurfaceId } from '../tokens';

type Theme = {
  feel: FeelId;
  surface: SurfaceId;
  lowMemory: boolean;
  /** The OS asked for reduced motion. */
  reducedMotion: boolean;
  /** Skip movement: reduced motion or low-memory mode. */
  calm: boolean;
  setFeel: (f: FeelId) => void;
  setSurface: (s: SurfaceId) => void;
  setLowMemory: (on: boolean) => void;
};

const ThemeContext = createContext<Theme | null>(null);
const STORAGE_KEY = 'awa.theme';

type Saved = { feel?: string; surface?: string; lowMemory?: boolean };

function loadSaved(): Saved {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Saved;
  } catch {
    return {};
  }
}

function initial(): { feel: FeelId; surface: SurfaceId; lowMemory: boolean } {
  const saved = loadSaved();
  const q = new URLSearchParams(location.search);
  const feel = q.get('feel') ?? saved.feel;
  const surface = q.get('surface') ?? saved.surface;
  const low = q.get('lowmem');
  return {
    feel: isFeelId(feel) ? feel : (tokens.defaults.feel as FeelId),
    surface: isSurfaceId(surface) ? surface : (tokens.defaults.surface as SurfaceId),
    lowMemory: low !== null ? low === '1' : (saved.lowMemory ?? tokens.defaults.lowMemory),
  };
}

function useReducedMotion(): boolean {
  const query = '(prefers-reduced-motion: reduce)';
  const [reduced, setReduced] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const on = () => setReduced(mq.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return reduced;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState(initial);
  const reducedMotion = useReducedMotion();

  // Variables live on <html> so popovers and portals inherit them.
  useLayoutEffect(() => {
    const root = document.documentElement;
    for (const [k, v] of Object.entries(themeVars(state.feel, state.surface, state.lowMemory))) root.style.setProperty(k, v);
    root.dataset.surface = state.surface;
    root.dataset.lowmem = String(state.lowMemory);
    root.style.colorScheme = tokens.surfaces[state.surface].scheme;
  }, [state]);

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
      // Storage can be unavailable (private mode); the theme still works for this session.
    }
  }, [state]);

  const value = useMemo<Theme>(
    () => ({
      ...state,
      reducedMotion,
      calm: reducedMotion || state.lowMemory,
      setFeel: (feel) => setState((s) => ({ ...s, feel })),
      setSurface: (surface) => setState((s) => ({ ...s, surface })),
      setLowMemory: (lowMemory) => setState((s) => ({ ...s, lowMemory })),
    }),
    [state, reducedMotion],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): Theme {
  const t = useContext(ThemeContext);
  if (!t) throw new Error('useTheme outside ThemeProvider');
  return t;
}
