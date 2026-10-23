import { type ButtonHTMLAttributes } from 'react';
import { Icon, type IconName } from './Icon';

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string; active?: boolean; size?: number };

export function IconButton({ icon, label, active, size = 18, className = '', ...rest }: IconButtonProps) {
  return (
    <button type="button" aria-label={label} title={label} {...rest} className={`icon-btn ${active ? 'is-active' : ''} ${className}`}>
      <Icon name={icon} size={size} />
    </button>
  );
}
