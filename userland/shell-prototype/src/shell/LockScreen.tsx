import { useEffect, useRef, useState, type FormEvent } from 'react';
import { fmtLongDate, fmtTime, useClock } from '../motion/useClock';
import { Backdrop } from '../scenes/Backdrop';
import { useTheme } from '../theme/ThemeProvider';
import { sceneCopy, tokens } from '../tokens';
import { Icon } from '../ui/Icon';

type Props = { user: string; onUnlock: () => void };

/**
 * Lock screen in the HORIZON composition: the time sits inside the eclipse
 * ring, the editorial "AWA OS" block in the corner. Any non-empty password
 * unlocks (there are no real accounts yet); an empty one shows the error state.
 */
export function LockScreen({ user, onUnlock }: Props) {
  const { feel } = useTheme();
  const copy = sceneCopy(feel);
  const now = useClock();
  const [value, setValue] = useState('');
  const [state, setState] = useState<'idle' | 'checking' | 'error'>('idle');
  const field = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => input.current?.focus(), []);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (state === 'checking') return;
    if (!value) {
      setState('error');
      field.current?.classList.remove('shake');
      void field.current?.offsetWidth; // restart the shake animation
      field.current?.classList.add('shake');
      return;
    }
    setState('checking');
    window.setTimeout(onUnlock, 450);
  };

  return (
    <main className="lock">
      <Backdrop feel={feel} className="lock-backdrop" />
      <div className="brand lock-brand">
        <div className="brand-title">AWA OS</div>
        <div className="brand-tagline">
          {(copy?.tagline ?? ['A new era', 'of computing', 'is here.']).map((line) => <span key={line}>{line}</span>)}
        </div>
      </div>
      {copy && (
        <div className="scene-number" aria-label={`Scene ${copy.number} of ${tokens.sceneCount}`}>
          {String(copy.number).padStart(2, '0')} <span className="counter-sep">/</span> {String(tokens.sceneCount).padStart(2, '0')}
        </div>
      )}
      <div className={`lock-clock ${copy?.clock === 'side' ? 'is-side' : ''}`}>
        <div className="lock-time" aria-label={`Time ${fmtTime(now)}`}>{fmtTime(now)}</div>
        <div className="eyebrow lock-date">{fmtLongDate(now)}</div>
      </div>
      <form className="lock-form" onSubmit={submit}>
        <div className="eyebrow lock-user">{user}</div>
        <div ref={field} className={`lock-field ${state === 'error' ? 'is-error' : ''}`}>
          <label htmlFor="lock-pass" className="sr-only">Password or key</label>
          <input
            id="lock-pass"
            ref={input}
            type="password"
            placeholder="Password or key"
            value={value}
            autoComplete="current-password"
            aria-invalid={state === 'error'}
            aria-describedby="lock-hint"
            onChange={(e) => {
              setValue(e.target.value);
              if (state === 'error') setState('idle');
            }}
          />
          <button type="submit" className="lock-go" aria-label="Unlock" disabled={state === 'checking'}>
            {state === 'checking' ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
          </button>
        </div>
        <div id="lock-hint" className={`lock-hint ${state === 'error' ? 'is-error' : ''}`} aria-live="polite">
          {state === 'error' ? 'Enter your password or key to unlock.' : state === 'checking' ? 'Unlocking…' : 'Your session keeps nothing outside its boxes'}
        </div>
      </form>
    </main>
  );
}
