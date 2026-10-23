import { type ReactNode } from 'react';
import { Icon, type IconName } from './Icon';

export function EmptyState({ icon, title, children }: { icon: IconName; title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <Icon name={icon} size={30} />
      <div className="empty-title">{title}</div>
      {children && <div className="empty-body">{children}</div>}
    </div>
  );
}

export function ErrorNote({ title, children, action }: { title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="error-note" role="alert">
      <Icon name="alert" size={18} />
      <div>
        <div className="error-title">{title}</div>
        {children && <div className="error-body">{children}</div>}
        {action && <div className="error-action">{action}</div>}
      </div>
    </div>
  );
}
