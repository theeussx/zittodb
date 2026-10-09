// Screen / scrcpy integration (spec §8, §14, §17).

import { useEffect, useRef, useState } from 'react';
import { BITRATE_OPTIONS, FPS_OPTIONS, ORIENTATIONS, SCRCPY_PRESETS, SIZE_OPTIONS, type ScrcpyPresetId } from '../../config/app';
import { getBridge } from '../../services/bridge';
import { act } from '../../services/deviceService';
import { useApp } from '../../stores/app';
import { Badge, Button } from '../../components/ui';
import { ErrorDialog } from '../../components/ConfirmDialog';
import type { AppError, ScrcpyOptions, ScrcpyStatus } from '../../types';

export function ScreenPanel({ serial }: { serial: string }) {
  const t = useApp((s) => s.t);
  const tools = useApp((s) => s.tools);
  const scrcpyTool = tools.find((x) => x.name === 'scrcpy');
  const available = !!scrcpyTool?.found;

  const [preset, setPreset] = useState<ScrcpyPresetId>('low');
  const [maxSize, setMaxSize] = useState<number | ''>(1280);
  const [maxFps, setMaxFps] = useState<number | ''>(30);
  const [bitrate, setBitrate] = useState<string>('2M');
  const [orientation, setOrientation] = useState<string>('auto');
  const [turnScreenOff, setTurnScreenOff] = useState(false);
  const [audio, setAudio] = useState(false);
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [record, setRecord] = useState(false);
  const [recordPath, setRecordPath] = useState('');

  const [status, setStatus] = useState<ScrcpyStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [seconds, setSeconds] = useState(0);
  const recStart = useRef(0);

  useEffect(() => {
    const off = getBridge().on('scrcpy-log', (payload) => {
      const p = payload as { serial: string; line: string };
      setLog((prev) => [...prev.slice(-199), p.line]);
    });
    return off;
  }, []);

  useEffect(() => {
    if (status?.recording) {
      recStart.current = Date.now();
      const t = setInterval(() => {
        setSeconds(Math.floor((Date.now() - recStart.current) / 1000));
      }, 1000);
      return () => clearInterval(t);
    }
    setSeconds(0);
  }, [status?.recording]);

  const applyPreset = (id: ScrcpyPresetId) => {
    setPreset(id);
    const p = SCRCPY_PRESETS[id];
    if (id === 'custom') return;
    setMaxSize(p.maxSize ?? '');
    setMaxFps(p.maxFps ?? '');
    setBitrate(p.bitrate ?? '');
  };

  const buildOptions = (effectiveRecordPath: string | null): ScrcpyOptions => ({
    preset,
    maxSize: maxSize === '' ? null : Number(maxSize),
    maxFps: maxFps === '' ? null : Number(maxFps),
    bitrate: bitrate || null,
    orientation,
    turnScreenOff,
    audio,
    alwaysOnTop,
    recordPath: record ? effectiveRecordPath : null,
  });

  const start = async () => {
    const effectiveRecordPath = record ? recordPath || (await getBridge().recordingFilename()) : null;
    if (effectiveRecordPath && effectiveRecordPath !== recordPath) setRecordPath(effectiveRecordPath);
    setBusy(true);
    setError(null);
    const r = await act(() => getBridge().scrcpyStart(serial, buildOptions(effectiveRecordPath)), {
      title: t('screen.running'),
    });
    if (r.ok) setStatus(r.value);
    else setError(r.error);
    setBusy(false);
  };

  const stop = async () => {
    const r = await act(() => getBridge().scrcpyStop());
    if (r.ok) setStatus(r.value);
    else setError(r.error);
  };

  const mm = String(Math.floor(seconds / 60)).padStart(2, '0');
  const ss = String(seconds % 60).padStart(2, '0');

  return (
    <div className="grid cols-2">
      <div className="card">
        <h3>{t('screen.preset')}</h3>
        <div className="row wrap" style={{ gap: 8 }}>
          {(Object.keys(SCRCPY_PRESETS) as ScrcpyPresetId[]).map((id) => (
            <Badge
              key={id}
              tone={preset === id ? 'ok' : 'default'}
            >
              <button
                className="btn ghost small"
                style={{ border: 0, background: 'transparent' }}
                onClick={() => applyPreset(id)}
              >
                {t(`screen.presets.${id}`)}
              </button>
            </Badge>
          ))}
        </div>

        <div className="grid cols-2 mt-12">
          <label className="field">
            {t('screen.resolution')}
            <select
              value={maxSize === '' ? '' : String(maxSize)}
              onChange={(e) => {
                setMaxSize(e.target.value === '' ? '' : Number(e.target.value));
                setPreset('custom');
              }}
            >
              <option value="">{t('screen.native')}</option>
              {SIZE_OPTIONS.map((s) => (
                <option key={s} value={s}>
                  {s}px
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {t('screen.fps')}
            <select
              value={String(maxFps)}
              onChange={(e) => {
                setMaxFps(e.target.value === '' ? '' : Number(e.target.value));
                setPreset('custom');
              }}
            >
              <option value="">{t('screen.native')}</option>
              {FPS_OPTIONS.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {t('screen.bitrate')}
            <select
              value={bitrate}
              onChange={(e) => {
                setBitrate(e.target.value);
                setPreset('custom');
              }}
            >
              <option value="">—</option>
              {BITRATE_OPTIONS.map((b) => (
                <option key={b} value={b}>
                  {b}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {t('screen.orientation')}
            <select value={orientation} onChange={(e) => setOrientation(e.target.value)}>
              {ORIENTATIONS.map((o) => (
                <option key={o} value={o}>
                  {t(`screen.orientation.${o}`)}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="grid cols-3 mt-12">
          <label className="check">
            <input type="checkbox" checked={turnScreenOff} onChange={(e) => setTurnScreenOff(e.target.checked)} />
            {t('screen.turnScreenOff')}
          </label>
          <label className="check">
            <input type="checkbox" checked={audio} onChange={(e) => setAudio(e.target.checked)} />
            {t('screen.audio')}
          </label>
          <label className="check">
            <input type="checkbox" checked={alwaysOnTop} onChange={(e) => setAlwaysOnTop(e.target.checked)} />
            {t('screen.alwaysOnTop')}
          </label>
        </div>

        <div className="mt-12">
          <label className="check">
            <input type="checkbox" checked={record} onChange={(e) => setRecord(e.target.checked)} />
            🔴 {t('screen.record')}
          </label>
          {record && (
            <div className="row mt-8">
              <input
                type="text"
                value={recordPath}
                onChange={(e) => setRecordPath(e.target.value)}
                placeholder="~/Videos/Zittodb/recording-....mp4"
              />
              <Button
                size="small"
                variant="ghost"
                onClick={async () => setRecordPath(await getBridge().recordingFilename())}
              >
                {t('screen.record.browse')}
              </Button>
            </div>
          )}
          <div className="dim small mt-8">{t('screen.record.hint')}</div>
        </div>

        <div className="row mt-16">
          {!status?.running ? (
            <Button variant="primary" onClick={start} disabled={busy || !available}>
              {t('screen.start')}
            </Button>
          ) : (
            <Button variant="danger" onClick={stop} disabled={busy}>
              ■ {t('screen.stop')}
            </Button>
          )}
          {status?.recording && (
            <Badge tone="danger">
              <span className="record-dot" aria-hidden />
              {t('screen.recording')} {mm}:{ss}
            </Badge>
          )}
          {status?.running && !status.recording && (
            <Badge tone="ok">
              {t('screen.running')} {status.pid ? `· pid ${status.pid}` : ''}
            </Badge>
          )}
        </div>
        {!available && (
          <div className="banner mt-12" style={{ margin: '12px 0 0' }}>
            ⚠ {t('screen.notAvailable')}
          </div>
        )}
      </div>

      <div className="card" style={{ display: 'flex', flexDirection: 'column', minHeight: 300 }}>
        <h3>{t('screen.log')}</h3>
        <div className="log-view" style={{ flex: 1 }} aria-live="polite">
          {log.length === 0 ? (
            <span className="dim">{t('screen.hint')}</span>
          ) : (
            log.map((line, i) => <div key={i}>{line}</div>)
          )}
        </div>
      </div>

      {error && <ErrorDialog error={error} onClose={() => setError(null)} />}
    </div>
  );
}
