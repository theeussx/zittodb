// Scanner Center: real checks against this computer or one selected device.
// Every result shows its evidence; checks that could NOT be evaluated are shown
// as such, because "no findings" is not proof of safety.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { asAppError, getBridge } from '../../services/bridge';
import { resolveLanguage } from '../../i18n';
import { useApp } from '../../stores/app';
import { Badge, Button, Spinner } from '../../components/ui';
import { ErrorDialog } from '../../components/ConfirmDialog';
import { SEVERITIES } from '../../types';
import type { AppError, CheckStatus, Finding, ScanDiff, ScanReport, ScanSummary, ScanTarget, Severity } from '../../types';

const TONE: Record<Severity, 'danger' | 'warn' | 'info' | 'default'> = {
  critical: 'danger',
  high: 'danger',
  medium: 'warn',
  low: 'info',
  info: 'default',
};

export function ScannerView() {
  const t = useApp((s) => s.t);
  const devices = useApp((s) => s.devices);
  const language = useApp((s) => s.settings.language);
  const toast = useApp((s) => s.toast);

  const [target, setTarget] = useState<ScanTarget>('environment');
  const [serial, setSerial] = useState('');
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<ScanReport | null>(null);
  const [history, setHistory] = useState<ScanSummary[]>([]);
  const [diff, setDiff] = useState<ScanDiff | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [severity, setSeverity] = useState<'all' | Severity>('all');
  const [category, setCategory] = useState('all');
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState<Set<string>>(new Set());
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const loadHistory = useCallback(async () => {
    try {
      const h = await getBridge().scanHistory();
      if (mounted.current) setHistory(h);
    } catch (e) {
      if (mounted.current) setError(asAppError(e));
    }
  }, []);

  useEffect(() => { void loadHistory(); }, [loadHistory]);

  // Default to the first listed device; the user can pick any (even unauthorized ones).
  useEffect(() => {
    if (!serial && devices.length) setSerial(devices[0].serial);
  }, [devices, serial]);

  const run = async () => {
    setRunning(true);
    setDiff(null);
    try {
      const r = await getBridge().scanRun(target, target === 'device' ? serial : null, resolveLanguage(language));
      if (!mounted.current) return;
      setReport(r);
      setSeverity('all');
      setCategory('all');
      setQuery('');
      setOpen(new Set());
      void loadHistory();
    } catch (e) {
      if (mounted.current) setError(asAppError(e));
    } finally {
      if (mounted.current) setRunning(false);
    }
  };

  const openScan = async (id: string) => {
    try {
      const r = await getBridge().scanGet(id);
      if (!mounted.current) return;
      setReport(r);
      setDiff(null);
      setOpen(new Set());
    } catch (e) {
      if (mounted.current) setError(asAppError(e));
    }
  };

  const remove = async (id: string) => {
    try {
      await getBridge().scanDelete(id);
      if (report?.scanId === id) setReport(null);
      await loadHistory();
    } catch (e) {
      setError(asAppError(e));
    }
  };

  // Newest earlier scan of the same target and subject.
  const previous = useMemo(() => {
    if (!report) return null;
    return (
      history
        .filter((h) => h.scanId !== report.scanId && h.target === report.target && h.subject === report.subject && h.startedAt <= report.startedAt)
        .sort((a, b) => b.startedAt - a.startedAt)[0] ?? null
    );
  }, [history, report]);

  const compare = async () => {
    if (!report || !previous) return;
    try {
      const d = await getBridge().scanCompare(previous.scanId, report.scanId);
      if (mounted.current) setDiff(d);
    } catch (e) {
      setError(asAppError(e));
    }
  };

  const exportAs = async (format: 'json' | 'markdown') => {
    if (!report) return;
    try {
      const dir = await getBridge().pickPath('directory');
      if (!dir) return;
      const text = await getBridge().scanExport(report.scanId, format);
      const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
      const path = `${dir.replace(/[\\/]+$/, '')}${sep}zittodb-${report.scanId}.${format === 'json' ? 'json' : 'md'}`;
      await getBridge().saveLogFile(path, text);
      toast({ kind: 'success', title: t('scan.exported'), body: path });
    } catch (e) {
      setError(asAppError(e));
    }
  };

  const categories = useMemo(() => [...new Set((report?.findings ?? []).map((f) => f.category))].sort(), [report]);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (report?.findings ?? []).filter(
      (f) =>
        (severity === 'all' || f.severity === severity) &&
        (category === 'all' || f.category === category) &&
        (!q || [f.title, f.ruleId, f.id, f.impact, f.recommendation, ...f.evidence.map((e) => `${e.key} ${e.value}`)].join(' ').toLowerCase().includes(q)),
    );
  }, [report, severity, category, query]);

  const toggle = (id: string) =>
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const canRun = !running && (target === 'environment' || !!serial);

  return (
    <div className="stack" style={{ gap: 16 }}>
      <div className="card">
        <h3>{t('scan.title')}</h3>
        <p className="muted">{t('scan.readonly')}</p>
        <div className="row wrap">
          <label>
            {t('scan.target')}{' '}
            <select aria-label={t('scan.target')} value={target} onChange={(e) => setTarget(e.target.value as ScanTarget)} disabled={running}>
              <option value="environment">{t('scan.target.environment')}</option>
              <option value="device">{t('scan.target.device')}</option>
            </select>
          </label>
          {target === 'device' &&
            (devices.length ? (
              <label>
                {t('scan.device')}{' '}
                <select aria-label={t('scan.device')} value={serial} onChange={(e) => setSerial(e.target.value)} disabled={running}>
                  {devices.map((d) => (
                    <option key={d.serial} value={d.serial}>
                      {d.model ?? d.serial} ({d.serial}, {d.state})
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <span className="muted">{t('scan.noDevice')}</span>
            ))}
          <Button onClick={run} disabled={!canRun}>{running ? t('scan.running') : t('scan.run')}</Button>
          {running && <Spinner label={t('scan.running')} />}
        </div>
      </div>

      <div className="row wrap" style={{ alignItems: 'flex-start', gap: 16 }}>
        <div className="card" style={{ minWidth: 240, flex: '0 0 260px' }}>
          <h3>{t('scan.history')}</h3>
          {history.length === 0 && <p className="muted">{t('scan.history.empty')}</p>}
          <ul className="plain-list" aria-label={t('scan.history')}>
            {history.map((h) => (
              <li key={h.scanId} className="row">
                <button className="link-button" onClick={() => void openScan(h.scanId)} aria-current={report?.scanId === h.scanId}>
                  {new Date(h.startedAt * 1000).toLocaleString()} · {t(`scan.target.${h.target}.short`)}
                  {h.subject ? ` · ${h.subject}` : ''} · {h.total}
                </button>
                <button className="link-button" aria-label={`${t('scan.delete')} ${h.scanId}`} onClick={() => void remove(h.scanId)}>✕</button>
              </li>
            ))}
          </ul>
        </div>

        <div style={{ flex: 1, minWidth: 320 }}>
          {!report && <div className="card muted">{t('scan.empty')}</div>}
          {report && (
            <div className="stack" style={{ gap: 12 }}>
              <div className="card">
                <div className="row wrap">
                  <strong>{t('scan.summary.findings', { n: report.summary.total })}</strong>
                  {SEVERITIES.filter((s) => report.summary.bySeverity[s]).map((s) => (
                    <Badge key={s} tone={TONE[s]}>{t(`scan.severity.${s}`)}: {report.summary.bySeverity[s]}</Badge>
                  ))}
                  <span className="muted">
                    {t('scan.summary.evaluated', { n: report.summary.checksEvaluated })} · {t('scan.summary.notEvaluated', { n: report.summary.checksNotEvaluated })}
                  </span>
                  <span className="spacer" />
                  <Button onClick={() => void exportAs('json')}>{t('scan.export.json')}</Button>
                  <Button onClick={() => void exportAs('markdown')}>{t('scan.export.md')}</Button>
                </div>
                <p className="muted" role="note">{t('scan.notProof')}</p>
                <div className="row wrap">
                  <Button onClick={compare} disabled={!previous}>{t('scan.compare')}</Button>
                  {!previous && <span className="muted">{t('scan.compare.none')}</span>}
                </div>
              </div>

              {diff && (
                <div className="card" aria-label={t('scan.diff.title')}>
                  <h3>{t('scan.diff.title')}</h3>
                  <p>
                    {t('scan.diff.counts', { n: diff.new.length, r: diff.resolved.length, c: diff.changed.length, u: diff.unchanged })}
                  </p>
                  <ul>
                    {diff.new.map((f) => <li key={f.id}>＋ {f.title}</li>)}
                    {diff.resolved.map((f) => <li key={f.id}>✓ {f.title}</li>)}
                    {diff.changed.map((c) => <li key={c.id}>⇄ {c.title}: {t(`scan.severity.${c.from}`)} → {t(`scan.severity.${c.to}`)}</li>)}
                  </ul>
                  <p className="muted">{t('scan.diff.note')}</p>
                </div>
              )}

              <div className="card">
                <div className="row wrap">
                  <select aria-label={t('scan.filter.severity')} value={severity} onChange={(e) => setSeverity(e.target.value as 'all' | Severity)}>
                    <option value="all">{t('scan.filter.all')}</option>
                    {SEVERITIES.map((s) => <option key={s} value={s}>{t(`scan.severity.${s}`)}</option>)}
                  </select>
                  <select aria-label={t('scan.filter.category')} value={category} onChange={(e) => setCategory(e.target.value)}>
                    <option value="all">{t('scan.filter.all')}</option>
                    {categories.map((c) => <option key={c} value={c}>{c}</option>)}
                  </select>
                  <input aria-label={t('scan.filter.search')} placeholder={t('scan.filter.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
                </div>
                {shown.length === 0 && <p className="muted">{report.findings.length ? t('scan.noMatch') : t('scan.noFindings')}</p>}
                <ul className="plain-list">
                  {shown.map((f) => (
                    <FindingRow key={f.id} f={f} open={open.has(f.id)} onToggle={() => toggle(f.id)} />
                  ))}
                </ul>
              </div>

              <details className="card">
                <summary>{t('scan.checks')}</summary>
                <ul className="plain-list">
                  {report.checks.map((c) => (
                    <li key={c.id}>
                      <Badge tone={checkTone(c.status)}>{t(`scan.check.${c.status}`)}</Badge> {c.title}
                      {c.detail && <span className="muted"> — {c.detail}</span>}
                    </li>
                  ))}
                </ul>
              </details>

              <div className="card">
                <h3>{t('scan.limitations')}</h3>
                <ul>{report.limitations.map((l) => <li key={l}>{l}</li>)}</ul>
              </div>
            </div>
          )}
        </div>
      </div>
      {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
    </div>
  );
}

function checkTone(s: CheckStatus): 'ok' | 'warn' | 'danger' | 'default' {
  return s === 'passed' ? 'ok' : s === 'found' ? 'warn' : s === 'error' ? 'danger' : 'default';
}

function FindingRow({ f, open, onToggle }: { f: Finding; open: boolean; onToggle: () => void }) {
  const t = useApp((s) => s.t);
  return (
    <li className="card" style={{ margin: '8px 0' }}>
      <button className="link-button" onClick={onToggle} aria-expanded={open}>
        <Badge tone={TONE[f.severity]}>{t(`scan.severity.${f.severity}`)}</Badge> <strong>{f.title}</strong>
      </button>
      {open && (
        <dl className="stack" style={{ gap: 6 }}>
          <div><dt className="muted">{t('scan.field.id')}</dt><dd><code>{f.id}</code> · <code>{f.ruleId}</code> · {f.category} · {t('scan.confidence')}: {t(`scan.conf.${f.confidence}`)}</dd></div>
          <div><dt className="muted">{t('scan.field.subject')}</dt><dd><code>{f.subject}</code></dd></div>
          <div><dt className="muted">{t('scan.field.detection')}</dt><dd>{f.detection}</dd></div>
          <div>
            <dt className="muted">{t('scan.field.evidence')}</dt>
            <dd>
              <ul>{f.evidence.map((e, i) => <li key={i}><code>{e.source}</code> · <code>{e.key}</code> = <code>{e.value}</code></li>)}</ul>
            </dd>
          </div>
          <div><dt className="muted">{t('scan.field.impact')}</dt><dd>{f.impact}</dd></div>
          <div><dt className="muted">{t('scan.field.recommendation')}</dt><dd>{f.recommendation}</dd></div>
          {f.reversible !== null && <div><dt className="muted">{t('scan.field.reversible')}</dt><dd>{f.reversible ? t('scan.yes') : t('scan.no')}</dd></div>}
        </dl>
      )}
    </li>
  );
}
