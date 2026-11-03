import { useEffect, useState } from 'react';
import { Backdrop } from '../scenes/Backdrop';
import { useTheme } from '../theme/ThemeProvider';
import type { FeelId } from '../tokens';

/**
 * The feel's background. Switching feels cross-fades the new one in on top,
 * then drops the old layer so only one scene stays in memory. In low-memory
 * mode it swaps instantly.
 */
export function Wallpaper() {
  const { feel, calm } = useTheme();
  const [base, setBase] = useState<FeelId>(feel);
  const [incoming, setIncoming] = useState<FeelId | null>(null);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    if (feel === base) return;
    if (calm) {
      setBase(feel);
      setIncoming(null);
      return;
    }
    setIncoming(feel);
    setShown(false);
    const raf = requestAnimationFrame(() => requestAnimationFrame(() => setShown(true)));
    return () => cancelAnimationFrame(raf);
  }, [feel]);

  return (
    <>
      <div className="wall-layer"><Backdrop feel={base} /></div>
      {incoming && (
        <div
          className={`wall-layer is-next ${shown ? 'is-in' : ''}`}
          onTransitionEnd={() => {
            setBase(incoming);
            setIncoming(null);
          }}
        >
          <Backdrop feel={incoming} />
        </div>
      )}
    </>
  );
}
