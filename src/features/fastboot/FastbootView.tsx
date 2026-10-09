// Fastboot (spec §28, §33): devices, safe read-only ops, gated destructive ops.

import { useCallback, useEffect, useState } from 'react';
import { getBridge } from '../../services/bridge';
import { act } from '../../services/deviceService';
import { useApp } from '../../stores/app';
import { Badge, Button, EmptyState, Spinner } from '../../components/ui';
import { DestructiveConfirmDialog, ErrorDialog, type DestructiveConfig } from '../../components/ConfirmDialog';
import type { AppError, FastbootDevice, FastbootOperation, OpResult } from '../../types';

export function FastbootView() {
  const t = useApp((s) => s.t);
  const [devices, setDevices] = useState<FastbootDevice[] | null>(null);
  const [selected, setSelected] = useState<string>('');
  const [result, setResult] = useState<OpResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<DestructiveConfig | null>(null);
  const [flashFile, setFlashFile] = useState('');
  const [getvar, setGetvar] = useState('product');

  const load = useCallback(async () => {
    setDevices(null);
    const r = await act(() => getBridge().fastbootDevices());
    if (r.ok) setDevices(r.value);
    else setError(r.error);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (op: FastbootOperation, confirmation?: string) => {
    setBusy(true);
    setError(null);
    setResult(null);
    const r = await act(() => getBridge().fastbootExecute(selected || null, op, confirmation));
    if (r.ok) setResult(r.value);
    else setError(r.error);
    setBusy(false);
  };

  const gate = (op: FastbootOperation, word: string, title: string, body: string) =>
    setConfirm({
      title,
      body,
      word,
      onConfirm: () => run(op, word),
      onCancel: () => setConfirm(null),
    });

  return (
    <div>
      <div className="card">
        <div className="row wrap">
          <h3 style={{ margin: 0 }}>{t('fb.devices')}</h3>
          <div className="spacer" />
          <Button size="small" onClick={load} disabled={devices === null}>
            {t('nav.refresh')}
          </Button>
        </div>
        {devices === null ? (
          <div className="mt-12">
            <Spinner label={t('fb.checking')} />
          </div>
        ) : devices.length === 0 ? (
          <div className="mt-12">
            <EmptyState icon="⚡" title={t('fb.empty')} hint={t('fb.empty.hint')} />
          </div>
        ) : (
          <div className="table-wrap mt-12">
            <table className="data">
              <thead>
                <tr>
                  <th></th>
                  <th>{t('fb.serial')}</th>
                  <th>{t('fb.state')}</th>
                </tr>
              </thead>
              <tbody>
                {devices.map((d) => (
                  <tr
                    key={d.serial}
                    className={`clickable ${selected === d.serial ? 'selected' : ''}`}
                    onClick={() => setSelected(d.serial)}
                  >
                    <td>
                      <input type="radio" checked={selected === d.serial} onChange={() => setSelected(d.serial)} />
                    </td>
                    <td className="mono">{d.serial}</td>
                    <td>
                      <Badge tone="info">{d.state}</Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <div className="grid cols-2 mt-12">
        <div className="card">
          <h3>{t('fb.operations')}</h3>
          <div className="grid" style={{ gap: 8 }}>
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => run({ op: 'getvar', var: getvar.trim() || 'product' })}
            >
              {t('fb.getvar')}
            </Button>
            <input
              type="text"
              value={getvar}
              onChange={(e) => setGetvar(e.target.value)}
              placeholder={t('fb.var')}
              className="mono"
              aria-label={t('fb.var')}
            />
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => run({ op: 'reboot', target: 'system' })}
            >
              {t('fb.reboot.system')}
            </Button>
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => run({ op: 'reboot', target: 'bootloader' })}
            >
              {t('fb.reboot.bootloader')}
            </Button>
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => run({ op: 'reboot', target: 'recovery' })}
            >
              {t('fb.reboot.recovery')}
            </Button>
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => gate({ op: 'unlock' }, 'APAGAR', t('fb.unlock'), t('fb.unlock.hint'))}
            >
              🔓 {t('fb.unlock')}
            </Button>
            <Button
              size="small"
              disabled={!selected || busy}
              onClick={() => gate({ op: 'lock' }, 'APAGAR', t('fb.lock'), t('fb.lock.hint'))}
            >
              🔒 {t('fb.lock')}
            </Button>
            <div className="row" style={{ gap: 8 }}>
              <input
                type="text"
                value={flashFile}
                onChange={(e) => setFlashFile(e.target.value)}
                placeholder="partition: image (ex: boot: boot.img)"
                className="mono"
                style={{ flex: 1 }}
              />
            </div>
            <Button
              size="small"
              variant="danger"
              disabled={!selected || busy || !flashFile}
              onClick={() => {
                const [part, img] = flashFile.split(':').map((x) => x.trim());
                if (part && img) {
                  gate({ op: 'flash', partition: part, file: img }, 'FLASHAR', t('fb.flash'), t('fb.flash.hint', { part, img }));
                }
              }}
            >
              ⚡ {t('fb.flash')}
            </Button>
            <Button
              size="small"
              variant="danger"
              disabled={!selected || busy}
              onClick={() => gate({ op: 'erase', partition: 'cache' }, 'APAGAR', t('fb.erase'), t('fb.erase.hint', { part: 'cache' }))}
            >
              {t('fb.erase')}
            </Button>
          </div>
          <div className="dim small mt-12">{t('fb.warning')}</div>
        </div>

        <div className="card">
          <h3>{t('builder.result')}</h3>
          {result ? (
            <pre className="cmd-box">{result.stdout || result.stderr || '—'}</pre>
          ) : (
            <div className="dim small">{t('fb.idle')}</div>
          )}
        </div>
      </div>

      {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
      {confirm && <DestructiveConfirmDialog cfg={confirm} />}
    </div>
  );
}
