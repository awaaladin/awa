import { useCallback, useEffect, useRef } from 'react';
import type { SpringConfig } from '../tokens';

type Values<K extends string> = Record<K, number>;

/**
 * Interruptible springs that write straight to the DOM (no re-render per
 * frame). Calling `to()` mid-flight keeps the current position and velocity
 * and aims at the new target, so a second click redirects the motion instead
 * of queueing it. With `reduced` the values jump to their targets.
 */
export function useSprings<K extends string>(
  initial: Values<K>,
  config: SpringConfig,
  apply: (v: Values<K>) => void,
  reduced: boolean,
) {
  const pos = useRef<Values<K>>({ ...initial });
  const vel = useRef<Values<K>>(Object.fromEntries(Object.keys(initial).map((k) => [k, 0])) as Values<K>);
  const target = useRef<Values<K>>({ ...initial });
  const frame = useRef<number | null>(null);
  const onRest = useRef<(() => void) | null>(null);
  const applyRef = useRef(apply);
  applyRef.current = apply;
  const cfgRef = useRef(config);
  cfgRef.current = config;

  const stop = () => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
  };

  const settle = useCallback(() => {
    stop();
    const done = onRest.current;
    onRest.current = null;
    done?.();
  }, []);

  const run = useCallback(() => {
    let last = performance.now();
    const keys = Object.keys(pos.current) as K[];
    const tick = (now: number) => {
      // Clamp long frames (background tab) so the integration stays stable.
      let remaining = Math.min((now - last) / 1000, 1 / 20);
      last = now;
      const { stiffness, damping, mass } = cfgRef.current;
      while (remaining > 0) {
        const dt = Math.min(1 / 240, remaining);
        remaining -= dt;
        for (const k of keys) {
          const a = (-stiffness * (pos.current[k] - target.current[k]) - damping * vel.current[k]) / mass;
          vel.current[k] += a * dt;
          pos.current[k] += vel.current[k] * dt;
        }
      }
      let moving = false;
      for (const k of keys) {
        const span = Math.max(1, Math.abs(target.current[k]));
        if (Math.abs(pos.current[k] - target.current[k]) > 0.0005 * span || Math.abs(vel.current[k]) > 0.002 * span) {
          moving = true;
        } else {
          pos.current[k] = target.current[k];
          vel.current[k] = 0;
        }
      }
      applyRef.current({ ...pos.current });
      if (moving) frame.current = requestAnimationFrame(tick);
      else settle();
    };
    stop();
    frame.current = requestAnimationFrame(tick);
  }, [settle]);

  const to = useCallback(
    (next: Partial<Values<K>>, done?: () => void) => {
      Object.assign(target.current, next);
      onRest.current = done ?? null;
      if (reduced) {
        Object.assign(pos.current, target.current);
        for (const k of Object.keys(vel.current) as K[]) vel.current[k] = 0;
        applyRef.current({ ...pos.current });
        settle();
        return;
      }
      run();
    },
    [reduced, run, settle],
  );

  /** Jump without animating (used to place things before they appear). */
  const set = useCallback((next: Partial<Values<K>>) => {
    Object.assign(pos.current, next);
    Object.assign(target.current, next);
    applyRef.current({ ...pos.current });
  }, []);

  useEffect(() => () => stop(), []);
  return { to, set, current: pos };
}
