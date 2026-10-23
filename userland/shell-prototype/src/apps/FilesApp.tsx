import { useState } from 'react';
import { useSystem } from '../state/SystemProvider';
import { Chip } from '../ui/Chip';
import { Icon, type IconName } from '../ui/Icon';
import { EmptyState } from '../ui/States';

const FOLDER_ICONS: Record<string, IconName> = { Home: 'home', Projects: 'folder', Datasets: 'dataset', Renders: 'frames', Downloads: 'download' };

export function FilesApp({ onOpenBoxes }: { onOpenBoxes: () => void }) {
  const { state } = useSystem();
  const folders = Object.keys(state.folders);
  const [folder, setFolder] = useState('Home');
  const [selected, setSelected] = useState<string | null>(null);
  const items = state.folders[folder] ?? [];

  return (
    <>
      <nav className="side" aria-label="Places">
        {folders.map((f) => (
          <button key={f} type="button" className="side-btn" aria-current={folder === f ? 'page' : undefined} onClick={() => { setFolder(f); setSelected(null); }}>
            <Icon name={FOLDER_ICONS[f] ?? 'folder'} size={16} />
            {f}
          </button>
        ))}
      </nav>
      <div className="pane">
        <h2>{folder}</h2>
        <p className="lede">Each item says how AWA may treat it when memory is tight.</p>
        {items.length === 0 ? (
          folder === 'Downloads' ? (
            <EmptyState icon="download" title="Nothing here, on purpose">
              Downloads never land in your home folder. They stop at the download broker and go into a box you choose.
              <div style={{ marginTop: 12 }}>
                <button type="button" className="btn btn-secondary btn-sm" onClick={onOpenBoxes}>Open Boxes</button>
              </div>
            </EmptyState>
          ) : (
            <EmptyState icon="folder" title="This folder is empty" />
          )
        ) : (
          <div className="file-grid stagger" role="listbox" aria-label={`${folder} contents`} style={{ marginTop: 18 }} key={folder}>
            {items.map((f, i) => (
              <button
                key={f.name}
                type="button"
                role="option"
                aria-selected={selected === f.name}
                className="file-card"
                style={{ ['--i' as string]: i }}
                onClick={() => setSelected(f.name)}
              >
                <Icon name={f.icon === 'folder' ? 'folder' : f.icon} size={30} stroke="var(--acc-text)" strokeWidth={1.6} />
                <div className="file-name">{f.name}</div>
                <div className="muted" style={{ fontSize: 11.5, marginTop: 2 }}>{f.size}</div>
                <div style={{ marginTop: 8 }}><Chip tone="accent">{f.memClass}</Chip></div>
              </button>
            ))}
          </div>
        )}
      </div>
    </>
  );
}
