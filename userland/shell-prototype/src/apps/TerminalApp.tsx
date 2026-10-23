import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { runCommand, type Line } from '../state/commands';
import { useSystem } from '../state/SystemProvider';
import { reduce } from '../state/system';

type Entry = { kind: 'in'; text: string } | ({ kind: 'out' } & Line);

const WELCOME: Entry[] = [{ kind: 'out', text: 'AWA shell. Type help for commands.', tone: 'muted' }];

/** A real prompt: the AWA commands run against the shared mock system state. */
export function TerminalApp({ onOpenBroker, focused }: { onOpenBroker: () => void; focused: boolean }) {
  const { state, dispatch } = useSystem();
  const [log, setLog] = useState<Entry[]>(WELCOME);
  const [input, setInput] = useState('');
  const [history, setHistory] = useState<string[]>([]);
  const [cursor, setCursor] = useState<number | null>(null);
  const out = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);

  useEffect(() => {
    out.current?.scrollTo({ top: out.current.scrollHeight });
  }, [log]);
  useEffect(() => {
    if (focused) field.current?.focus({ preventScroll: true });
  }, [focused]);

  // Review aid: ?run=ram show;gpu show types these commands on open.
  const ran = useRef(false);
  useEffect(() => {
    const script = new URLSearchParams(location.search).get('run');
    if (!script || ran.current) return;
    ran.current = true;
    let s = state;
    const entries: Entry[] = [];
    for (const text of script.split(';').map((c) => c.trim()).filter(Boolean)) {
      const result = runCommand(text, s);
      for (const a of result.actions) {
        dispatch(a);
        s = reduce(s, a); // later commands in the script see earlier changes
      }
      entries.push({ kind: 'in', text }, ...result.lines.map((line) => ({ kind: 'out' as const, ...line })));
      if (result.openBroker) onOpenBroker();
    }
    setLog((l) => [...l, ...entries]);
  }, []);

  const submit = () => {
    const text = input.trim();
    setInput('');
    setCursor(null);
    if (!text) {
      setLog((l) => [...l, { kind: 'in', text: '' }]);
      return;
    }
    setHistory((h) => [...h, text]);
    const result = runCommand(text, state);
    for (const a of result.actions) dispatch(a);
    if (result.clear) {
      setLog([]);
      return;
    }
    setLog((l) => [...l, { kind: 'in', text }, ...result.lines.map((line) => ({ kind: 'out' as const, ...line }))]);
    if (result.openBroker) onOpenBroker();
  };

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') submit();
    else if (e.key === 'ArrowUp' && history.length) {
      e.preventDefault();
      const next = cursor === null ? history.length - 1 : Math.max(0, cursor - 1);
      setCursor(next);
      setInput(history[next] ?? '');
    } else if (e.key === 'ArrowDown' && cursor !== null) {
      e.preventDefault();
      const next = cursor + 1;
      if (next >= history.length) {
        setCursor(null);
        setInput('');
      } else {
        setCursor(next);
        setInput(history[next] ?? '');
      }
    } else if (e.key === 'l' && e.ctrlKey) {
      e.preventDefault();
      setLog([]);
    }
  };

  return (
    <div className="term" onClick={() => window.getSelection()?.isCollapsed && field.current?.focus()}>
      <div className="term-out" ref={out} role="log" aria-live="polite" aria-label="Terminal output">
        {log.map((e, i) =>
          e.kind === 'in' ? (
            <div key={i}><span className="prompt">awa ›</span> {e.text}</div>
          ) : (
            <div key={i} className={e.tone ? `t-${e.tone}` : undefined}>{e.text || ' '}</div>
          ),
        )}
      </div>
      <div className="term-in">
        <label htmlFor="term-input" className="prompt">awa ›</label>
        <input
          id="term-input"
          ref={field}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onKey}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="off"
          aria-label="Command"
        />
      </div>
    </div>
  );
}
