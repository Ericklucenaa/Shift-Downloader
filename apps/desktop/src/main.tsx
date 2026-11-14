import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles.css';

// Menu de contexto do navegador não faz sentido num app desktop (exceto em campos de texto).
document.addEventListener('contextmenu', (e) => {
  const el = e.target as HTMLElement;
  if (!el.closest('input, textarea, .selectable')) e.preventDefault();
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
