import { type ButtonHTMLAttributes } from 'react';
import { Icon, type IconName } from './Icon';

type TileProps = ButtonHTMLAttributes<HTMLButtonElement> & { title: string; detail?: string; on?: boolean; icon?: IconName };

/** Quick-settings style toggle tile. */

export function Tile({ title, detail, on, icon, className = '', ...rest }: TileProps) {
  return (
    <button type="button" aria-pressed={on} {...rest} className={`tile ${on ? 'is-on' : ''} ${className}`}>
      {icon && <Icon name={icon} size={18} />}
      <span className="tile-text">
        <span className="tile-title">{title}</span>
        {detail && <span className="tile-detail">{detail}</span>}
      </span>
    </button>
  );
}
