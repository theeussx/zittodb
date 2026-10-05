import { useState } from 'react';
import type { UpdateInfo } from '../services/updateService';
import { useApp } from '../stores/app';

export function UpdateBanner({ update, onDismiss }: { update: UpdateInfo; onDismiss: () => void }) {
  const t = useApp((s) => s.t);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState(0);

  const install = async () => {
    if (!update.install || installing) return;
    setInstalling(true);
    try {
      await update.install((event) => {
        if (event.event === 'Started') setProgress(0);
        if (event.event === 'Progress') setProgress((value) => Math.min(99, value + 1));
      });
    } catch {
      setInstalling(false);
    }
  };

  return (
    <div className="banner update-banner" role="status">
      <span aria-hidden>↻</span>
      <span>
        <strong>{t('updates.available', { version: update.version })}</strong>
        {update.name ? ` — ${update.name}` : ''}
        {' · '}
        <a href={update.url} target="_blank" rel="noreferrer">
          {t('updates.viewRelease')}
        </a>
        {update.install && (
          <>
            {' · '}
            <button className="btn primary small" type="button" onClick={() => void install()} disabled={installing}>
              {installing ? t('updates.installing', { progress }) : t('updates.install')}
            </button>
          </>
        )}
      </span>
      <button className="btn ghost small" type="button" onClick={onDismiss} disabled={installing} aria-label={t('updates.dismiss')}>
        {t('updates.dismiss')}
      </button>
    </div>
  );
}
