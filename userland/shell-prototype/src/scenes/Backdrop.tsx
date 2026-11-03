import { tokens, WALLPAPERS, type FeelId } from '../tokens';
import { FlowScene } from './FlowScene';
import { HorizonScene } from './HorizonScene';
import { MonolithScene } from './MonolithScene';
import { OrbitScene } from './OrbitScene';
import { SignalScene } from './SignalScene';

/** Draws a feel's background: a live scene (HORIZON, ORBIT) or one of the wallpaper images. */
export function Backdrop({ feel, className = '' }: { feel: FeelId; className?: string }) {
  const art = tokens.feels[feel].wallpaper;
  if (art === 'scene:horizon') return <HorizonScene className={`backdrop ${className}`} />;
  if (art === 'scene:orbit') return <OrbitScene className={`backdrop ${className}`} />;
  if (art === 'scene:flow') return <FlowScene className={`backdrop ${className}`} />;
  if (art === 'scene:signal') return <SignalScene className={`backdrop ${className}`} />;
  if (art === 'scene:monolith') return <MonolithScene className={`backdrop ${className}`} />;
  return <div className={`backdrop ${className}`} style={{ backgroundImage: `url("${WALLPAPERS[feel]}")` }} aria-hidden="true" />;
}
