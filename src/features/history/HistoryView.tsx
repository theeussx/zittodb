// History (spec §13, §31): audit trail + last-seen devices. Local only.

import { useCallback, useEffect, useState } from 'react';
import { getBridge } from '../../services/bridge';
import { act } from '../../services/deviceService';
import { useApp } from '../../stores/app';
import { Badge, Button, EmptyState, Spinner } from '../../components/ui';
import { ErrorDialog } from '../../components/ConfirmDialog';
import type { AppError, AuditEntry, HistoryEntry, UndoRef } from '../../types';

function timeAgo(
  ms: number | null,
  t: (k: string, p?: Record<string, string | number>) => string,
): string {
  if (!ms) return '—';
  const s = Math.floor((Date.now() - ms) / 1000);
  if (s < 60) return t('history.justNow');
  if (s < 3600) return t('history.minutesAgo', { n: Math.floor(s / 60) });
  if (s < 86400) return t('history.hoursAgo', { n: Math.floor(s / 3600) });
  return new Date(ms).toLocaleDateString('pt-BR');
}

export function HistoryView() {
  const t = useApp((s) => s.t);
  const [tab, setTab] = useState<'audit' | 'devices'>('audit');
  const [audit, setAudit] = useState<AuditEntry[] | null>(null);
  const [devices, setDevices] = useState<HistoryEntry[] | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [lastUndo, setLastUndo] = useState<UndoRef | null>(null);
  const [editingSerial, setEditingSerial] = useState<string | null>(null);
  const [aliasDraft, setAliasDraft] = useState('');
  const [tagsDraft, setTagsDraft] = useState('');

  const load = useCallback(async () => {
    const [a, d] = await Promise.all([
      act(() => getBridge().getAudit(300)),
      act(() => getBridge().getDeviceHistory()),
    ]);
    if (a.ok) setAudit(a.value);
    if (d.ok) setDevices(d.value);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const undo = async (entry: AuditEntry) => {
    if (!entry.undo) return;
    const ref = entry.undo;
    setLastUndo(ref);
    const pkgOp =
      ref.action === 'enable_package'
        ? ({ op: 'enable_package', pkg: ref.package } as const)
        : ({ op: 'reinstall_existing', pkg: ref.package } as const);
    const r = await act(() => getBridge().executeOperation(entry.device, pkgOp));
    if (!r.ok) setError(r.error);
    void load();
  };

  const editDevice = (entry: HistoryEntry) => {
    setEditingSerial(entry.serial);
    setAliasDraft(entry.alias ?? '');
    setTagsDraft(entry.tags.join(', '));
  };
  const saveDevice = async (entry: HistoryEntry) => {
    const result = await act(() => getBridge().updateDeviceMetadata(
      entry.serial,
      aliasDraft.trim() || null,
      tagsDraft.split(',').map((tag) => tag.trim()).filter(Boolean),
      entry.favorite,
    ));
    if (result.ok) {
      setDevices(result.value);
      setEditingSerial(null);
    } else setError(result.error);
  };
  const toggleFavorite = async (entry: HistoryEntry) => {
    const result = await act(() => getBridge().updateDeviceMetadata(entry.serial, entry.alias ?? null, entry.tags, !entry.favorite));
    if (result.ok) setDevices(result.value);
    else setError(result.error);
  };

  return (
    <div>
      <div className="tabs">
        <button className={`tab ${tab === 'audit' ? 'active' : ''}`} onClick={() => setTab('audit')}>
          {t('history.audit')}
        </button>
        <button className={`tab ${tab === 'devices' ? 'active' : ''}`} onClick={() => setTab('devices')}>
          {t('history.devices')}
        </button>
        <div className="spacer" />
        <Button size="small" variant="ghost" onClick={load}>
          {t('nav.refresh')}
        </Button>
        {tab === 'audit' && audit && audit.length > 0 && (
          <Button
            size="small"
            variant="ghost"
            onClick={async () => {
              await getBridge().clearAudit();
              setAudit([]);
            }}
          >
            {t('history.clearAudit')}
          </Button>
        )}
        {tab === 'devices' && devices && devices.length > 0 && (
          <Button
            size="small"
            variant="ghost"
            onClick={async () => {
              await getBridge().clearDeviceHistory();
              setDevices([]);
            }}
          >
            {t('history.clearDevices')}
          </Button>
        )}
      </div>

      {tab === 'audit' &&
        (audit === null ? (
          <div className="card">
            <Spinner />
          </div>
        ) : audit.length === 0 ? (
          <EmptyState icon="🕘" title={t('history.auditEmpty')} />
        ) : (
          <div className="table-wrap" style={{ maxHeight: 'calc(100vh - 220px)' }}>
            <table className="data">
              <thead>
                <tr>
                  <th>{t('history.time')}</th>
                  <th>{t('history.device')}</th>
                  <th>{t('history.action')}</th>
                  <th>{t('history.command')}</th>
                  <th>{t('history.result')}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {audit.map((a, i) => (
                  <tr key={i}>
                    <td className="dim">{new Date(a.ts).toLocaleTimeString('pt-BR')}</td>
                    <td className="mono">{a.device ?? '—'}</td>
                    <td>{a.action}</td>
                    <td className="mono" title={a.command}>
                      {a.command.length > 60 ? a.command.slice(0, 60) + '…' : a.command}
                    </td>
                    <td>
                      <Badge tone={a.result === 'ok' ? 'ok' : 'danger'}>{a.result}</Badge>
                    </td>
                    <td style={{ textAlign: 'right' }}>
                      {a.undo && (
                        <Button
                          size="small"
                          variant="ghost"
                          onClick={() => void undo(a)}
                          disabled={lastUndo !== null && lastUndo.package === a.undo!.package}
                        >
                          {t('history.undo')}
                        </Button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}

      {tab === 'devices' &&
        (devices === null ? (
          <div className="card">
            <Spinner />
          </div>
        ) : devices.length === 0 ? (
          <EmptyState icon="🕘" title={t('history.devicesEmpty')} />
        ) : (
          <div className="table-wrap">
            <table className="data">
              <thead>
                <tr>
                  <th>{t('history.device.serial')}</th>
                  <th>{t('history.device.alias')}</th>
                  <th>{t('history.device.model')}</th>
                  <th>{t('history.device.tags')}</th>
                  <th>{t('history.device.favorite')}</th>
                  <th>{t('history.device.state')}</th>
                  <th>{t('history.device.lastSeen')}</th>
                  <th>{t('history.device.conn')}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {devices.map((d) => (
                  <tr key={d.serial}>
                    <td className="mono">{d.serial}</td>
                    <td>{editingSerial === d.serial ? <input value={aliasDraft} onChange={(e) => setAliasDraft(e.target.value)} /> : (d.alias ?? '—')}</td>
                    <td>{d.model ?? '—'}</td>
                    <td>{editingSerial === d.serial ? <input value={tagsDraft} onChange={(e) => setTagsDraft(e.target.value)} placeholder={t('history.device.tagsHint')} /> : (d.tags.length ? d.tags.join(', ') : '—')}</td>
                    <td><Button size="small" variant="ghost" onClick={() => void toggleFavorite(d)}>{d.favorite ? '★' : '☆'}</Button></td>
                    <td>
                      <Badge tone={d.lastState === 'connected' ? 'ok' : 'default'}>{d.lastState ?? '—'}</Badge>
                    </td>
                    <td>{timeAgo(d.lastSeenMs, t)}</td>
                    <td>{d.connection ?? '—'}</td>
                    <td>{editingSerial === d.serial ? <Button size="small" onClick={() => void saveDevice(d)}>{t('common.save')}</Button> : <Button size="small" variant="ghost" onClick={() => editDevice(d)}>{t('history.device.edit')}</Button>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}

      {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
    </div>
  );
}
