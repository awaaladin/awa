import { useState } from 'react';
import { BootSequence } from './shell/BootSequence';
import { Desktop } from './shell/Desktop';
import { LockScreen } from './shell/LockScreen';
import { isAppId, type OpenRequest, type SettingsSection } from './shell/apps';
import { StateGallery } from './dev/StateGallery';
import { useSystem } from './state/SystemProvider';

type Screen = 'boot' | 'lock' | 'desktop' | 'gallery';

/**
 * Boot -> lock -> desktop. URL parameters jump straight to a state (used for
 * review screenshots): ?screen=desktop&apps=terminal,settings&section=memory
 * &feel=ember&surface=white&lowmem=1&time=20:21, or ?screen=gallery.
 */
function fromUrl(): { screen: Screen; apps?: OpenRequest[] } {
  const q = new URLSearchParams(location.search);
  const s = q.get('screen');
  const screen: Screen = s === 'lock' || s === 'desktop' || s === 'gallery' ? s : 'boot';
  const section = (['appearance', 'memory', 'gpu'] as const).find((x) => x === q.get('section')) as SettingsSection | undefined;
  const apps = q.get('apps')?.split(',').filter(isAppId).map((app) => ({ app, section: app === 'settings' ? section : undefined }));
  return { screen, apps };
}

export function App() {
  const { state } = useSystem();
  const [{ screen, apps }, setNav] = useState(fromUrl);
  const go = (next: Screen) => setNav((n) => ({ ...n, screen: next }));

  if (screen === 'gallery') return <StateGallery />;
  if (screen === 'boot') return <BootSequence onDone={() => go('lock')} />;
  if (screen === 'lock') return <LockScreen user={state.user} onUnlock={() => go('desktop')} />;
  return <Desktop initial={apps && apps.length ? apps : undefined} />;
}
