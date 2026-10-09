// Debloat (spec §9, §18, §54): risk classification, profiles, batch ops, reversibility.

import { useCallback, useEffect, useMemo, useState } from 'react';
import { getBridge } from '../../services/bridge';
import { act } from '../../services/deviceService';
import { useApp } from '../../stores/app';
import { Badge, Button, EmptyState, Spinner } from '../../components/ui';
import { DestructiveConfirmDialog, ErrorDialog, type DestructiveConfig } from '../../components/ConfirmDialog';
import type { AppError, BatchResult, DebloatProfile, PackageRow, RiskLevel } from '../../types';

const RISK_ORDER: RiskLevel[] = ['SAFE', 'LOW_RISK', 'CAUTION', 'DANGEROUS', 'CRITICAL', 'UNKNOWN'];

function riskBadge(_r: RiskLevel, label: string, tone: 'ok' | 'warn' | 'danger' | 'info' | 'default') {
  return (
    <Badge tone={tone}>
      {label}
    </Badge>
  );
}

function riskTone(r: RiskLevel): { tone: 'ok' | 'warn' | 'danger' | 'info' | 'default'; labelKey: string } {
  switch (r) {
    case 'SAFE':
      return { tone: 'ok', labelKey: 'debloat.risk.SAFE' };
    case 'LOW_RISK':
      return { tone: 'info', labelKey: 'debloat.risk.LOW_RISK' };
    case 'CAUTION':
      return { tone: 'warn', labelKey: 'debloat.risk.CAUTION' };
    case 'DANGEROUS':
      return { tone: 'danger', labelKey: 'debloat.risk.DANGEROUS' };
    case 'CRITICAL':
      return { tone: 'danger', labelKey: 'debloat.risk.CRITICAL' };
    default:
      return { tone: 'default', labelKey: 'debloat.risk.UNKNOWN' };
  }
}

export function DebloatView({ serial }: { serial: string }) {
  const t = useApp((s) => s.t);
  const toast = useApp((s) => s.toast);

  const [rows, setRows] = useState<PackageRow[] | null>(null);
  const [risks, setRisks] = useState<Record<string, RiskLevel>>({});
  const [profiles, setProfiles] = useState<DebloatProfile[]>([]);
  const [profile, setProfile] = useState('conservative');
  const [operation, setOperation] = useState<'disable' | 'uninstall'>('disable');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [confirm, setConfirm] = useState<DestructiveConfig | null>(null);
  const [batch, setBatch] = useState<BatchResult | null>(null);

  const load = useCallback(async () => {
    setRows(null);
    setRisks({});
    const r = await act(() => getBridge().listPackages(serial));
    if (!r.ok) {
      setError(r.error);
      return;
    }
    setRows(r.value);
    const pkgList = r.value.map((x) => x.name);
    const cls = await act(() => getBridge().classifyPackages(pkgList));
    if (cls.ok) {
      const map: Record<string, RiskLevel> = {};
      for (const c of cls.value) map[c.package] = c.risk;
      setRisks(map);
    }
    const profs = await act(() => getBridge().debloatProfiles());
    if (profs.ok) setProfiles(profs.value);
  }, [serial]);

  useEffect(() => {
    void load();
  }, [load]);

  const activeProfile = profiles.find((p) => p.id === profile) ?? null;
  const included = useMemo(() => {
    if (!activeProfile || !rows) return [];
    return rows.filter((r) => {
      if (r.isDisabled) return false;
      const rk = risks[r.name] ?? 'UNKNOWN';
      const max = RISK_ORDER.indexOf(activeProfile.maxRisk);
      const cur = RISK_ORDER.indexOf(rk);
      return cur <= max && cur >= 0;
    });
  }, [activeProfile, rows, risks]);

  const toggle = (pkg: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(pkg)) next.delete(pkg);
      else next.add(pkg);
      return next;
    });
  };

  const selectAllIncluded = () => setSelected(new Set(included.map((r) => r.name)));

  const runBatch = () => {
    const pkgs = [...selected];
    if (pkgs.length === 0) return;
    setConfirm({
      title: t('debloat.batchTitle'),
      body: t('debloat.batchBody', { n: pkgs.length, pkgs: pkgs.slice(0, 5).join(', ') + (pkgs.length > 5 ? '…' : '') }),
      word: operation === 'uninstall' ? 'REMOVER' : 'APAGAR',
      onConfirm: async () => {
        setBusy(true);
        setBatch(null);
        const op = operation === 'uninstall'
          ? { op: 'uninstall_for_user' as const, pkg: pkgs[0] ?? '', user: 0 }
          : { op: 'disable_package' as const, pkg: pkgs[0] ?? '', user: 0 };
        const confirmation = operation === 'uninstall' ? 'REMOVER' : 'APAGAR';
        const r = await act(() =>
          getBridge().executeBatch(serial, op, pkgs, confirmation),
        );
        if (r.ok) {
          setBatch(r.value);
          toast({ kind: 'success', title: t('debloat.batchDone') });
          setSelected(new Set());
          void load();
        } else setError(r.error);
        setBusy(false);
      },
      onCancel: () => setConfirm(null),
    });
  };

  if (rows === null) {
    return (
      <div className="card">
        <Spinner label={t('debloat.loading')} />
      </div>
    );
  }

  const visible = rows
    .filter((r) => !r.isDisabled)
    .sort((a, b) => {
      const ra = RISK_ORDER.indexOf(risks[a.name] ?? 'UNKNOWN');
      const rb = RISK_ORDER.indexOf(risks[b.name] ?? 'UNKNOWN');
      return ra === rb ? a.name.localeCompare(b.name) : ra - rb;
    });

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, minHeight: 0 }}>
      <div className="card">
        <div className="row wrap">
          <label className="field">
            {t('debloat.profile')}
            <select value={profile} onChange={(e) => setProfile(e.target.value)} style={{ maxWidth: 220 }}>
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <div className="small muted" style={{ alignSelf: 'center' }}>
            {activeProfile?.description || t('debloat.profileHint', { profile: activeProfile?.name ?? '' })}
          </div>
          <div className="spacer" />
          <Button size="small" onClick={load} disabled={busy}>
            {t('nav.refresh')}
          </Button>
        </div>
        <div className="row wrap mt-12">
          {RISK_ORDER.map((r) => {
            const { tone, labelKey } = riskTone(r);
            return riskBadge(r, t(labelKey), tone);
          })}
        </div>
        <div className="dim small mt-8">{t('debloat.legality')}</div>
      </div>

      {batch && (
        <div className="card">
          <h3>{t('debloat.batchResult')}</h3>
          {batch.results.map((x) => (
            <div key={x.package} className="row small" style={{ marginBottom: 4 }}>
              <Badge tone={x.ok ? 'ok' : 'danger'}>{x.ok ? 'OK' : 'ERRO'}</Badge>
              <span className="mono">{x.package}</span>
              {x.message && <span className="dim">{x.message}</span>}
            </div>
          ))}
          <div className="dim small mt-8">{t('debloat.undoHint')}</div>
        </div>
      )}

      <div className="row">
        <Button size="small" onClick={selectAllIncluded} disabled={included.length === 0}>
          {t('debloat.selectAll', { n: included.length })}
        </Button>
        <label className="field" style={{ minWidth: 220 }}>
          {t('debloat.op')}
          <select value={operation} onChange={(e) => setOperation(e.target.value as 'disable' | 'uninstall')} disabled={busy}>
            <option value="disable">{t('debloat.op.disable')}</option>
            <option value="uninstall">{t('debloat.op.uninstall')}</option>
          </select>
        </label>
        <Button variant="primary" size="small" onClick={runBatch} disabled={selected.size === 0 || busy}>
          {t('debloat.execute', { n: selected.size })}
        </Button>
        <div className="spacer" />
        <span className="dim small">{t('debloat.hint')}</span>
      </div>

      {visible.length === 0 ? (
        <EmptyState icon="✂" title={t('debloat.empty')} />
      ) : (
        <div className="table-wrap" style={{ flex: 1, maxHeight: 460 }}>
          <table className="data">
            <thead>
              <tr>
                <th></th>
                <th>{t('apps.col.package')}</th>
                <th>{t('debloat.risk')}</th>
                <th>{t('apps.col.type')}</th>
                <th>{t('debloat.inProfile')}</th>
              </tr>
            </thead>
            <tbody>
              {visible.map((r) => {
                const rk = risks[r.name] ?? 'UNKNOWN';
                const { tone, labelKey } = riskTone(rk);
                const inProfile = included.some((x) => x.name === r.name);
                return (
                  <tr key={r.name}>
                    <td>
                      <input
                        type="checkbox"
                        checked={selected.has(r.name)}
                        onChange={() => toggle(r.name)}
                        aria-label={r.name}
                      />
                    </td>
                    <td className="mono">{r.name}</td>
                    <td>{riskBadge(rk, t(labelKey), tone)}</td>
                    <td>{t(`apps.type.${r.isSystem ? 'system' : 'user'}`)}</td>
                    <td>{inProfile ? '✓' : ''}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
      {confirm && <DestructiveConfirmDialog cfg={confirm} />}
    </div>
  );
}
