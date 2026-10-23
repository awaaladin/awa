import { useId } from 'react';

type SliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
  format?: (v: number) => string;
  disabled?: boolean;
  hideLabel?: boolean;
  /** Shorter text shown on screen; `label` stays the accessible name. */
  shortLabel?: string;
};

export function Slider({ label, value, min, max, step, onChange, format, disabled, hideLabel, shortLabel }: SliderProps) {
  const id = useId();
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className={`slider ${disabled ? 'is-disabled' : ''}`}>
      <label htmlFor={id} className={hideLabel ? 'sr-only' : 'slider-label'}>{shortLabel ?? label}</label>
      <input
        id={id}
        aria-label={shortLabel ? label : undefined}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        aria-valuetext={format ? format(value) : undefined}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ ['--pct' as string]: `${pct}%` }}
      />
      {format && <output htmlFor={id} className="slider-value">{format(value)}</output>}
    </div>
  );
}
