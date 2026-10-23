import type { IconName } from '../ui/Icon';

export type AppId = 'files' | 'terminal' | 'browser' | 'boxes' | 'settings';
export type SettingsSection = 'appearance' | 'memory' | 'gpu';

export const APPS: Record<AppId, { title: string; icon: IconName }> = {
  files: { title: 'Files', icon: 'folder' },
  terminal: { title: 'Terminal', icon: 'terminal' },
  browser: { title: 'Browser', icon: 'browser' },
  boxes: { title: 'Boxes', icon: 'box' },
  settings: { title: 'Settings', icon: 'settings' },
};

export const APP_IDS = Object.keys(APPS) as AppId[];

export const isAppId = (v: string): v is AppId => v in APPS;

/** Opening an app, optionally at a place inside it. */
export type OpenRequest = { app: AppId; section?: SettingsSection };
