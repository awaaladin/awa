import raw from '@theme/awa-tokens.json';

import nexus from '@design/03-original-awa-wallpapers/01-NEXUS-eclipse.svg';
import prism from '@design/03-original-awa-wallpapers/02-PRISM-glass.svg';
import flow from '@design/03-original-awa-wallpapers/03-FLOW-light-strands.svg';
import signal from '@design/03-original-awa-wallpapers/04-SIGNAL-rings.svg';
import monolith from '@design/03-original-awa-wallpapers/05-MONOLITH-slab.svg';
import ember from '@design/03-original-awa-wallpapers/06-EMBER-dragon.svg';
import aurora from '@design/03-original-awa-wallpapers/07-AURORA-night.svg';
import tide from '@design/03-original-awa-wallpapers/08-TIDE-coast.svg';

export { default as logoIcon } from '@design/04-logo/awa-logo-icon.svg';
export { default as logoWordmark } from '@design/04-logo/awa-wordmark.svg';

export const tokens = raw;

export type FeelId = keyof typeof raw.feels;
export type SurfaceId = keyof typeof raw.surfaces;
export type Feel = (typeof raw.feels)[FeelId];
export type SpringName = keyof typeof raw.motion.spring;
export type SpringConfig = { stiffness: number; damping: number; mass: number };

export const FEEL_IDS = Object.keys(raw.feels) as FeelId[];
export const SURFACE_IDS = Object.keys(raw.surfaces) as SurfaceId[];

export const WALLPAPERS: Record<FeelId, string> = { nexus, prism, flow, signal, monolith, ember, aurora, tide };

export const isFeelId = (v: unknown): v is FeelId => typeof v === 'string' && v in raw.feels;
export const isSurfaceId = (v: unknown): v is SurfaceId => typeof v === 'string' && v in raw.surfaces;

export function spring(name: SpringName): SpringConfig {
  return raw.motion.spring[name];
}

/**
 * Samples a mass-spring-damper going from 0 to 1 and returns it as a CSS
 * `linear()` easing plus its settle time. CSS transitions retarget from their
 * current value when interrupted, so hover and press feedback built on this
 * stays interruptible without JavaScript.
 */
export function springEasing(cfg: SpringConfig): { easing: string; duration: number } {
  const dt = 1 / 240;
  let x = 0;
  let v = 0;
  let t = 0;
  let settledFor = 0;
  const samples: number[] = [0];
  while (t < 2 && settledFor < 0.05) {
    const a = (-cfg.stiffness * (x - 1) - cfg.damping * v) / cfg.mass;
    v += a * dt;
    x += v * dt;
    t += dt;
    samples.push(x);
    settledFor = Math.abs(x - 1) < 0.001 && Math.abs(v) < 0.01 ? settledFor + dt : 0;
  }
  const points = 32;
  const out: string[] = [];
  for (let i = 0; i <= points; i++) {
    out.push((samples[Math.round((i / points) * (samples.length - 1))] ?? 1).toFixed(4));
  }
  out[out.length - 1] = '1';
  return { easing: `linear(${out.join(', ')})`, duration: Math.round(t * 1000) };
}

/** CSS custom properties for a feel + surface combination. */
export function themeVars(feelId: FeelId, surfaceId: SurfaceId, lowMemory: boolean): Record<string, string> {
  const f = raw.feels[feelId];
  const s = raw.surfaces[surfaceId];
  const vars: Record<string, string> = {
    '--acc': f.accent,
    '--acc2': f.accent2,
    '--on-acc': f.onAccent,
    '--acc-text': s.accentText === 'ink' ? f.ink : f.accent2,
    '--glass': s.glass === 'feel' ? f.glass : s.glass,
    '--win': s.window === 'feel' ? f.window : s.window,
    '--text': s.text,
    '--muted': s.muted,
    '--line': s.line,
    '--raised': s.raised,
    '--hover': s.hover,
    '--selected': s.selected,
    '--track': s.track,
    '--focus': s.focus,
    '--blur': `${lowMemory ? raw.blur.lowMemory : raw.blur.glass}px`,
    '--wall': `url("${WALLPAPERS[feelId]}")`,
    '--shadow-window': raw.shadow.window,
    '--shadow-window-inactive': raw.shadow.windowInactive,
    '--shadow-popover': raw.shadow.popover,
    '--shadow-raised': raw.shadow.raised,
    '--shadow-text': raw.shadow.text,
    '--font-heading': `'${raw.font.heading}', system-ui, sans-serif`,
    '--font-body': `'${raw.font.body}', system-ui, sans-serif`,
    '--font-mono': `'${raw.font.mono}', ui-monospace, monospace`,
    '--r-xs': `${raw.radius.xs}px`,
    '--r-sm': `${raw.radius.sm}px`,
    '--r-md': `${raw.radius.md}px`,
    '--r-lg': `${raw.radius.lg}px`,
    '--r-xl': `${raw.radius.xl}px`,
    '--press-scale': String(raw.motion.pressScale),
    '--hover-lift': `${raw.motion.hoverLift}px`,
    '--ease-standard': raw.motion.easing.standard,
    '--dur-fast': `${raw.motion.duration.fast}ms`,
    '--dur-base': `${raw.motion.duration.base}ms`,
    '--stagger': `${raw.motion.stagger}ms`,
  };
  for (const name of ['press', 'snappy', 'panel'] as const) {
    const { easing, duration } = springEasing(raw.motion.spring[name]);
    vars[`--ease-${name}`] = easing;
    vars[`--dur-${name}`] = `${duration}ms`;
  }
  return vars;
}
