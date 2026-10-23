import { useId } from 'react';

type ToggleProps = { label: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; description?: string };

export function Toggle({ label, checked, onChange, disabled, description }: ToggleProps) {
  const id = useId();
  return (
    <div className={`toggle-row ${disabled ? 'is-disabled' : ''}`}>
      <div className="toggle-text">
        <label htmlFor={id} className="toggle-label">{label}</label>
        {description && <div className="toggle-desc" id={`${id}-d`}>{description}</div>}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-describedby={description ? `${id}-d` : undefined}
        disabled={disabled}
        className={`toggle ${checked ? 'is-on' : ''}`}
        onClick={() => onChange(!checked)}
      >
        <span className="toggle-knob" />
      </button>
    </div>
  );
}

/** Segmented single choice (radio group semantics). */
