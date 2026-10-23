import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { SystemProvider } from './state/SystemProvider';
import { ThemeProvider } from './theme/ThemeProvider';
import './styles.css';

const root = document.getElementById('root');
if (!root) throw new Error('missing #root');

createRoot(root).render(
  <StrictMode>
    <ThemeProvider>
      <SystemProvider>
        <App />
      </SystemProvider>
    </ThemeProvider>
  </StrictMode>,
);
