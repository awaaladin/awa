import {
  boxClass, boxUsed, fmtGB, parseSize, ramUsed, round, tradeoff, validBoxName, vramUsed,
  type Action, type SystemState,
} from './system';

export type Line = { text: string; tone?: 'muted' | 'accent' | 'error' };
export type CommandResult = { lines: Line[]; actions: Action[]; clear?: boolean; openBroker?: boolean };

const pad = (s: string, n: number) => (s.length >= n ? `${s} ` : s + ' '.repeat(n - s.length));

const HELP: Line[] = [
  { text: 'ram show                      memory per box', tone: 'muted' },
  { text: 'ram set <box> <size>          change a box limit, e.g. ram set render 2GB', tone: 'muted' },
  { text: 'gpu show                      GPU memory and jobs', tone: 'muted' },
  { text: 'box list | new <name> [--light] | run <name> <app> | reset <name>', tone: 'muted' },
  { text: 'get <url> --into <box>        download into a box', tone: 'muted' },
  { text: 'clear', tone: 'muted' },
];

const err = (text: string): CommandResult => ({ lines: [{ text, tone: 'error' }], actions: [] });

/** Runs one AWA shell command against the mock system state. */
export function runCommand(input: string, s: SystemState): CommandResult {
  const words = input.trim().split(/\s+/).filter(Boolean);
  const [cmd, sub, ...rest] = words;
  if (!cmd) return { lines: [], actions: [] };
  const findBox = (id: string | undefined) => s.boxes.find((b) => b.id === id);

  switch (cmd) {
    case 'help':
      return { lines: HELP, actions: [] };
    case 'clear':
      return { lines: [], actions: [], clear: true };

    case 'ram': {
      if (sub === 'show') {
        const lines: Line[] = [{ text: `${pad('BOX', 10)}${pad('LIMIT', 9)}${pad('USED', 9)}CLASS`, tone: 'muted' }];
        for (const b of s.boxes) {
          lines.push({ text: `${pad(b.id, 10)}${pad(fmtGB(b.limitGB), 9)}${pad(fmtGB(boxUsed(b)), 9)}${boxClass(b)}` });
        }
        lines.push({ text: `total     ${fmtGB(ramUsed(s))} of ${fmtGB(s.ramTotalGB)} in use · measured by AWA`, tone: 'muted' });
        return { lines, actions: [] };
      }
      if (sub === 'set') {
        const [name, size] = rest;
        if (!name || !size) return err('usage: ram set <box> <size>   e.g. ram set render 2GB');
        const box = findBox(name);
        if (!box) return err(`no box named "${name}". Try: box list`);
        const gb = parseSize(size);
        if (gb === null) return err(`"${size}" is not a size. Use e.g. 512MB, 2GB or 1.5G`);
        if (gb > s.ramTotalGB) return err(`${fmtGB(gb)} is more than this machine has (${fmtGB(s.ramTotalGB)}).`);
        const report = tradeoff(box, gb);
        if (!report.ok) return err(report.lines[0]?.text ?? 'invalid limit');
        const lines: Line[] = [{ text: `${box.id} limit is now ${fmtGB(gb)} (was ${fmtGB(box.limitGB)})`, tone: 'accent' }];
        for (const l of report.lines) lines.push({ text: `  ${l.source === 'measured' ? 'measured' : 'declared'}  ${l.text}`, tone: 'muted' });
        return { lines, actions: [{ type: 'setLimit', box: box.id, gb }] };
      }
      return err('usage: ram show | ram set <box> <size>');
    }

    case 'gpu': {
      if (sub !== 'show') return err('usage: gpu show');
      const lines: Line[] = [
        { text: `${s.gpu.name}   vram ${round(vramUsed(s), 1)} / ${s.gpu.vramTotalGB} GB   queue ${s.gpu.jobs.length} jobs` },
      ];
      for (const j of s.gpu.jobs) lines.push({ text: `  ${pad(j.box, 9)}${pad(j.label, 28)}${pad(j.sched, 13)}${fmtGB(j.vramGB)}`, tone: 'muted' });
      return { lines, actions: [] };
    }

    case 'box': {
      if (sub === 'list') {
        return {
          lines: [
            { text: `${pad('BOX', 10)}${pad('KIND', 7)}${pad('STATE', 10)}${pad('NETWORK', 9)}USED`, tone: 'muted' },
            ...s.boxes.map((b) => ({ text: `${pad(b.id, 10)}${pad(b.kind, 7)}${pad(b.state, 10)}${pad(b.network, 9)}${fmtGB(boxUsed(b))}` })),
          ],
          actions: [],
        };
      }
      if (sub === 'new') {
        const [name, flag] = rest;
        if (!name) return err('usage: box new <name> [--light]');
        if (!validBoxName(name)) return err('box names use a-z, 0-9 and -, start with a letter, max 24 characters');
        if (findBox(name)) return err(`a box named "${name}" already exists`);
        const kind = flag === '--light' ? 'light' : 'vm';
        return {
          lines: [{ text: `box "${name}" created (${kind === 'vm' ? 'VM box, disposable, network off' : 'light box'}). Starting…`, tone: 'accent' }],
          actions: [{ type: 'newBox', name, kind }],
        };
      }
      if (sub === 'run') {
        const [name, app] = rest;
        if (!name || !app) return err('usage: box run <name> <app>');
        if (!findBox(name)) return err(`no box named "${name}"`);
        return { lines: [{ text: `${app} is running in "${name}"`, tone: 'accent' }], actions: [{ type: 'runApp', box: name, app }] };
      }
      if (sub === 'reset') {
        const [name] = rest;
        const box = findBox(name);
        if (!name || !box) return err(name ? `no box named "${name}"` : 'usage: box reset <name>');
        if (box.id === 'system') return err('the system box cannot be reset');
        return { lines: [{ text: `"${name}" is back to a clean state. Its writable layer was discarded.`, tone: 'accent' }], actions: [{ type: 'resetBox', box: name }] };
      }
      return err('usage: box list | new <name> [--light] | run <name> <app> | reset <name>');
    }

    case 'get': {
      const url = sub;
      if (!url) return err('usage: get <url> --into <box>');
      try {
        const u = new URL(url);
        if (u.protocol !== 'https:' && u.protocol !== 'http:') return err('only http and https links can be downloaded');
      } catch {
        return err(`"${url}" is not a link`);
      }
      const i = rest.indexOf('--into');
      const target = i >= 0 ? rest[i + 1] : undefined;
      if (i >= 0 && !target) return err('usage: get <url> --into <box>');
      if (target && !findBox(target)) return err(`no box named "${target}". Try: box list`);
      if (!target) {
        return {
          lines: [{ text: 'No box given. The download broker is asking where it should go.', tone: 'muted' }],
          actions: [{ type: 'startDownload', url, box: null }],
          openBroker: true,
        };
      }
      return {
        lines: [{ text: `downloading into box "${target}" over 8 connections. Nothing touches your home folder.`, tone: 'accent' }],
        actions: [{ type: 'startDownload', url, box: target }],
      };
    }
  }
  return err(`unknown command "${cmd}". Type help`);
}
