import { useEffect, useRef, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';
import { useSprings } from '../motion/spring';
import { useTheme } from '../theme/ThemeProvider';
import { spring } from '../tokens';
import { IconButton } from '../ui/IconButton';

export type Rect = { x: number; y: number; w: number; h: number };

type Props = {
  title: string;
  rect: Rect;
  z: number;
  focused: boolean;
  maximized: boolean;
  minimized: boolean;
  closing: boolean;
  onFocus: () => void;
  onMove: (x: number, y: number) => void;
  onMinimize: () => void;
  onToggleMax: () => void;
  /** Starts the close animation (parent sets `closing`). */
  onClose: () => void;
  /** Called when the close animation has finished. */
  onClosed: () => void;
  children: ReactNode;
};

const TASKBAR_SPACE = 76;

/**
 * A desktop window. Opening, closing and minimising run on one interruptible
 * spring (`window` in the motion tokens): clicking the taskbar button while a
 * window is still minimising sends it straight back from where it is.
 */
export function WindowFrame(props: Props) {
  const { title, rect, z, focused, maximized, minimized, closing, onFocus, onMove, onMinimize, onToggleMax, onClose, onClosed, children } = props;
  const { calm } = useTheme();
  const el = useRef<HTMLElement>(null);
  const pos = useRef({ x: rect.x, y: rect.y });
  const drag = useRef<{ dx: number; dy: number; id: number } | null>(null);

  const paint = (p: number, m: number) => {
    const node = el.current;
    if (!node) return;
    const { x, y } = maximized ? { x: 0, y: 0 } : pos.current;
    const scale = (0.94 + 0.06 * p) * (1 - 0.3 * m);
    node.style.opacity = String(Math.max(0, Math.min(1, p * (1 - m))));
    node.style.transform = `translate(${x}px, ${y + (1 - p) * 14 + m * 280}px) scale(${scale})`;
    node.style.visibility = p * (1 - m) < 0.01 && (m > 0.99 || closing) ? 'hidden' : 'visible';
  };

  const springs = useSprings({ p: 0, m: minimized ? 1 : 0 }, spring('window'), ({ p, m }) => paint(p, m), calm);

  useEffect(() => {
    springs.to({ p: 1 });
  }, []);
  useEffect(() => {
    // Reopening mid-close reverses from wherever the window is.
    if (closing) springs.to({ p: 0 }, onClosed);
    else springs.to({ p: 1 });
  }, [closing]);
  useEffect(() => {
    springs.to({ m: minimized ? 1 : 0 });
    // A minimised window must not be reachable by Tab or screen readers.
    if (el.current) el.current.inert = minimized;
  }, [minimized]);
  useEffect(() => {
    pos.current = { x: rect.x, y: rect.y };
    paint(springs.current.current.p, springs.current.current.m);
  }, [rect.x, rect.y, maximized]);

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    onFocus();
    if (maximized || e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    drag.current = { dx: e.clientX - pos.current.x, dy: e.clientY - pos.current.y, id: e.pointerId };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    const maxX = window.innerWidth - 120;
    const maxY = window.innerHeight - TASKBAR_SPACE - 42;
    pos.current = {
      x: Math.min(maxX, Math.max(120 - rect.w, e.clientX - d.dx)),
      y: Math.min(maxY, Math.max(0, e.clientY - d.dy)),
    };
    paint(springs.current.current.p, springs.current.current.m);
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current?.id !== e.pointerId) return;
    drag.current = null;
    onMove(pos.current.x, pos.current.y);
  };

  const size = maximized
    ? { width: '100%', height: `calc(100% - ${TASKBAR_SPACE}px)` }
    : { width: rect.w, height: rect.h };

  return (
    <section
      ref={el}
      className={`window ${focused ? 'is-focused' : ''} ${maximized ? 'is-max' : ''}`}
      style={{ left: 0, top: 0, zIndex: z, opacity: 0, ...size }}
      aria-label={title}
      onPointerDownCapture={onFocus}
    >
      <div className="titlebar" onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerCancel={onPointerUp} onDoubleClick={onToggleMax}>
        <div className="window-title">
          <span className="title-dot" aria-hidden="true" />
          {title}
        </div>
        <div className="window-actions">
          <IconButton icon="minimize" label={`Minimise ${title}`} onClick={onMinimize} size={14} />
          <IconButton icon={maximized ? 'restore' : 'maximize'} label={maximized ? `Restore ${title}` : `Maximise ${title}`} onClick={onToggleMax} size={13} />
          <IconButton icon="close" label={`Close ${title}`} onClick={onClose} size={14} className="close" />
        </div>
      </div>
      <div className="window-body">{children}</div>
    </section>
  );
}
