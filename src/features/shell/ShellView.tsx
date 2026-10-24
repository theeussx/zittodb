// Shell terminal (spec §13, §25): persistent session per device, no auto-commands.

import { useEffect, useRef, useState } from 'react';
import { getBridge, asAppError } from '../../services/bridge';
import { useApp } from '../../stores/app';
import { Badge, Button, Spinner } from '../../components/ui';
import { DestructiveConfirmDialog, ErrorDialog } from '../../components/ConfirmDialog';
import { SHELL_CONFIRMATION_WORD } from '../../types';
import type { AppError, ShellAssessment } from '../../types';

interface Line {
  text: string;
  kind: 'prompt' | 'out' | 'err';
}

export function ShellView({ serial }: { serial: string }) {
  const t = useApp((s) => s.t);
  const [session, setSession] = useState<string | null>(null);
  const [lines, setLines] = useState<Line[]>([]);
  const [input, setInput] = useState('');
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(true);
  const [assessment, setAssessment] = useState<ShellAssessment | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<string | null>(null);
  sessionRef.current = session;

  useEffect(() => {
    let cancelled = false;
    let id: string | null = null;

    setLines([{ text: `${t('shell.opening', { serial})}…`, kind: 'out' }]);
    getBridge()
      .shellOpen(serial)
      .then((s) => {
        if (cancelled) {
          void getBridge().shellClose(s.id).catch(() => {});
          return;
        }
        id = s.id;
        sessionRef.current = s.id;
        setSession(s.id);
        setBusy(false);
      })
      .catch((e) => {
        if (!cancelled) { setError(asAppError(e)); setBusy(false); }
      });

    const off = getBridge().on('shell-output', (payload) => {
      const p = payload as { session: string; line: string };
      if (p.session !== sessionRef.current) return;
      setLines((prev) => {
        const next = [...prev];
        for (const rawLine of p.line.split('\n')) {
          if (rawLine.length === 0) continue;
          const kind = rawLine.startsWith('$ ') ? 'prompt' : rawLine.includes('inaccessible or not found') || rawLine.startsWith('sh: ') ? 'err' : 'out';
          next.push({ text: rawLine, kind });
        }
        return next.slice(-2000);
      });
    });

    return () => {
      cancelled = true;
      off();
      if (id) void getBridge().shellClose(id).catch(() => {});
    };
  }, [serial, t]);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [lines]);

  // Risk badge for what is being typed (debounced). The backend enforces the
  // gate regardless; this only informs the user before pressing Enter.
  useEffect(() => {
    const line = input.trim();
    if (!line) {
      setAssessment(null);
      return;
    }
    let stale = false;
    const timer = setTimeout(() => {
      getBridge()
        .shellClassify(line)
        .then((a) => { if (!stale) setAssessment(a); })
        // e.g. the browser dev server has no classifier: show no badge.
        .catch(() => { if (!stale) setAssessment(null); });
    }, 150);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [input]);

  const run = async (cmd: string, confirmation?: string) => {
    if (!session) return;
    setInput('');
    setAssessment(null);
    setLines((prev) => [...prev, { text: `$ ${cmd}`, kind: 'prompt' }]);
    try {
      await getBridge().shellWrite(session, cmd, confirmation);
    } catch (e) {
      setError(asAppError(e));
    }
  };

  const send = async () => {
    const cmd = input.trim();
    if (!cmd || !session) return;
    let verdict = assessment;
    try {
      // Re-classify right before sending: never trust a stale badge.
      verdict = await getBridge().shellClassify(cmd);
    } catch {
      verdict = null;
    }
    if (verdict?.risk === 'destructive') {
      setPending(cmd);
      return;
    }
    await run(cmd);
  };

  const clearView = () => setLines([]);

  return (
    <div className="grid cols-2" style={{ height: '100%', minHeight: 0 }}>
      <div className="terminal">
        <div className="row" style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)' }}>
          <span className="mono small">{serial}</span>
          <Badge tone={session ? 'ok' : 'warn'}>{session ? t('shell.connected') : t('shell.opening.short')}</Badge>
          <div className="spacer" />
          <Button size="small" variant="ghost" onClick={clearView} title={t('shell.clear')}>
            {t('shell.clear')}
          </Button>
        </div>
        {busy ? (
          <div style={{ padding: 14 }}>
            <Spinner label={t('shell.opening', { serial })} />
          </div>
        ) : (
          <div className="term-body" ref={bodyRef} aria-live="polite">
            {lines.map((l, i) => (
              <div key={i} className={`term-line-${l.kind === 'prompt' ? 'prompt' : l.kind === 'err' ? 'err' : 'out'}`}>
                {l.text}
              </div>
            ))}
          </div>
        )}
        <div className="term-input-row">
          <span className="mono" style={{ color: 'var(--accent)' }}>
            $
          </span>
          {assessment && (
            <span title={t('shell.risk.hint')}>
              <Badge tone={assessment.risk === 'read' ? 'ok' : assessment.risk === 'modify' ? 'warn' : 'danger'}>
                {t(`shell.risk.${assessment.risk}`)}
              </Badge>
            </span>
          )}
          <input
            className="term-input"
            value={input}
            disabled={!session}
            placeholder={t('shell.placeholder')}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void send();
            }}
          />
        </div>
      </div>

      <div className="card" style={{ alignSelf: 'start' }}>
        <h3>{t('shell.hintTitle')}</h3>
        <p className="small muted" style={{ margin: 0 }}>
          {t('shell.hint')}
        </p>
        <p className="dim small" style={{ margin: '8px 0 0' }}>
          {t('shell.security')}
        </p>
        {pending && (
          <DestructiveConfirmDialog
            cfg={{
              title: t('shell.confirm.title'),
              body: t('shell.confirm.body', { cmd: pending }),
              word: SHELL_CONFIRMATION_WORD,
              onCancel: () => setPending(null),
              onConfirm: async () => {
                const cmd = pending;
                setPending(null);
                await run(cmd, SHELL_CONFIRMATION_WORD);
              },
            }}
          />
        )}
        {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
      </div>
    </div>
  );
}
