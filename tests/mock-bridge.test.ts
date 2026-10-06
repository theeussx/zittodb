// Behavioral tests of the MockBridge — the same contract the Tauri bridge
// implements, so demo mode and tests exercise identical semantics.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { MockBridge } from '../src/services/mock';
import type { TransferEvent } from '../src/types';

describe('MockBridge: devices', () => {
  it('lists a realistic farm with mixed states', async () => {
    const b = new MockBridge();
    const list = await b.listDevices();
    expect(list.some((d) => d.state === 'connected' && d.connection === 'usb')).toBe(true);
    expect(list.some((d) => d.state === 'unauthorized')).toBe(true);
    expect(list.some((d) => d.state === 'offline')).toBe(true);
  });

  it('rejects operations on unauthorized devices', async () => {
    const b = new MockBridge();
    const list = await b.listDevices();
    const unauth = list.find((d) => d.state === 'unauthorized')!;
    await expect(b.getDeviceInfo(unauth.serial)).rejects.toMatchObject({
      code: 'DEVICE_UNAUTHORIZED',
    });
  });

  it('persists alias, tags and favorite metadata', async () => {
    const b = new MockBridge();
    const updated = await b.updateDeviceMetadata('23021RAA2Y', 'Telefone de QA', ['qa', 'lab'], false);
    expect(updated.find((d) => d.serial === '23021RAA2Y')).toMatchObject({
      alias: 'Telefone de QA',
      tags: ['qa', 'lab'],
      favorite: false,
    });
  });
});

describe('MockBridge: ADB server controls', () => {
  it('requires explicit confirmation before restart', async () => {
    const b = new MockBridge();
    await expect(b.adbServerRestart('')).rejects.toMatchObject({ code: 'CONFIRMATION_REQUIRED' });
    await expect(b.adbServerRestart('REINICIAR_ADB')).resolves.toContain('restarted');
  });
});

describe('MockBridge: destructive operations need typed words', () => {
  it('file delete without APAGAR → CONFIRMATION_REQUIRED', async () => {
    const b = new MockBridge();
    await expect(b.fileDelete('23021RAA2Y', '/sdcard/x', '')).rejects.toMatchObject({
      code: 'CONFIRMATION_REQUIRED',
    });
    await expect(b.fileDelete('23021RAA2Y', '/sdcard/x', 'APAGAR')).resolves.toMatchObject({
      ok: true,
    });
  });

  it('uninstall_for_user without REMOVER → CONFIRMATION_REQUIRED', async () => {
    const b = new MockBridge();
    await expect(
      b.packageAction('23021RAA2Y', 'com.example.promo', 'uninstall_for_user', 0, ''),
    ).rejects.toMatchObject({ code: 'CONFIRMATION_REQUIRED' });
  });

  it('reboot without REINICIAR → CONFIRMATION_REQUIRED', async () => {
    const b = new MockBridge();
    await expect(b.rebootDevice('23021RAA2Y', 'system', '')).rejects.toMatchObject({
      code: 'CONFIRMATION_REQUIRED',
    });
    await expect(b.rebootDevice('23021RAA2Y', 'system', 'REINICIAR')).resolves.toMatchObject({
      ok: true,
    });
  });

  it('fastboot flash without FLASHAR → CONFIRMATION_REQUIRED', async () => {
    const b = new MockBridge();
    await expect(
      b.fastbootExecute({ op: 'flash', partition: 'boot', file: '/tmp/boot.img' }, ''),
    ).rejects.toMatchObject({ code: 'CONFIRMATION_REQUIRED' });
  });

  it('non-destructive ops run without confirmation', async () => {
    const b = new MockBridge();
    const r = await b.executeOperation('23021RAA2Y', { op: 'get_props' });
    expect(r.ok).toBe(true);
  });
});

describe('MockBridge: events', () => {
  afterEach(() => vi.useRealTimers());

  it('shell session streams output', async () => {
    vi.useFakeTimers();
    const b = new MockBridge();
    const events: { session: string; line: string }[] = [];
    b.on('shell-output', (p) => events.push(p as { session: string; line: string }));

    const open = b.shellOpen('23021RAA2Y');
    await vi.advanceTimersByTimeAsync(500);
    const s = await open;
    expect(s.serial).toBe('23021RAA2Y');

    const write = b.shellWrite(s.id, 'getprop ro.product.model');
    await vi.advanceTimersByTimeAsync(500);
    await write;
    expect(events.some((e) => e.line.includes('Redmi Note 12'))).toBe(true);
  });

  it('logcat starts and can be stopped', async () => {
    vi.useFakeTimers();
    const b = new MockBridge();
    const lines: string[] = [];
    b.on('logcat-line', (p) => lines.push((p as { line: string }).line));

    const start = b.logcatStart('23021RAA2Y');
    await vi.advanceTimersByTimeAsync(500);
    const id = await start;
    await vi.advanceTimersByTimeAsync(1200);
    expect(lines.length).toBeGreaterThanOrEqual(2);

    await b.logcatStop(id);
    const after = lines.length;
    await vi.advanceTimersByTimeAsync(1200);
    expect(lines.length).toBe(after); // no more lines after stop
  });

  it('file pull reports running → done with byte counts', async () => {
    vi.useFakeTimers();
    const b = new MockBridge();
    const events: TransferEvent[] = [];
    b.on('file-progress', (p) => events.push(p as TransferEvent));

    const pull = b.filePull('23021RAA2Y', '/sdcard/Download/arquivo.zip');
    await vi.advanceTimersByTimeAsync(500);
    const id = await pull;
    await vi.advanceTimersByTimeAsync(1000);
    expect(events.some((e) => e.status === 'running' && e.id === id)).toBe(true);
    await vi.advanceTimersByTimeAsync(8000);
    const done = events.find((e) => e.id === id && e.status === 'done');
    expect(done).toBeTruthy();
    expect(done!.total).toBeGreaterThan(0);
    expect(done!.doneBytes).toBe(done!.total);
  });

  it('scrcpy refuses to start twice', async () => {
    const b = new MockBridge();
    await b.scrcpyStart('23021RAA2Y', {
      preset: 'low',
      maxSize: 1280,
      maxFps: 30,
      bitrate: '2M',
      orientation: 'auto',
      turnScreenOff: false,
      audio: false,
      alwaysOnTop: false,
      recordPath: null,
    });
    await expect(
      b.scrcpyStart('23021RAA2Y', {
        preset: 'low',
        maxSize: 1280,
        maxFps: 30,
        bitrate: '2M',
        orientation: 'auto',
        turnScreenOff: false,
        audio: false,
        alwaysOnTop: false,
        recordPath: null,
      }),
    ).rejects.toMatchObject({ code: 'ALREADY_RUNNING' });
    const st = await b.scrcpyStop();
    expect(st.running).toBe(false);
  });
});

describe('MockBridge: install APK (spec V0.1)', () => {
  it('installs a .apk path on a connected device', async () => {
    const b = new MockBridge();
    const r = await b.installApk('23021RAA2Y', '/home/user/app-release.apk');
    expect(r.ok).toBe(true);
    expect(r.stdout).toContain('Success');
  });

  it('rejects non-apk files with INVALID_ARGUMENT', async () => {
    const b = new MockBridge();
    await expect(b.installApk('23021RAA2Y', '/home/user/notes.txt')).rejects.toMatchObject({
      code: 'INVALID_ARGUMENT',
    });
  });

  it('describes install_apk as install -r', async () => {
    const b = new MockBridge();
    const desc = await b.describeOperation(
      { op: 'install_apk', local: '/tmp/app.apk' },
      '23021RAA2Y',
    );
    expect(desc).toContain("install -r");
  });
});

describe('MockBridge: screenshot naming (spec: never overwrite silently)', () => {
  it('names files screenshot-YYYY-MM-DD-HHMMSS.png', async () => {
    const b = new MockBridge();
    const r = await b.screenshot('23021RAA2Y', false);
    expect(r.path).toMatch(/screenshot-\d{4}-\d{2}-\d{2}-\d{6}\.png$/);
  });
});

describe('MockBridge: debloat + audit', () => {
  it('classifies critical packages and refuses to batch-disable them', async () => {
    const b = new MockBridge();
    const risks = await b.classifyPackages(['com.android.systemui', 'com.miui.analytics']);
    const sys = risks.find((r) => r.package === 'com.android.systemui')!;
    expect(sys.risk).toBe('CRITICAL');

    const res = await b.executeBatch(
      '23021RAA2Y',
      { op: 'disable_package', pkg: 'x', user: 0 },
      ['com.android.systemui', 'com.miui.analytics'],
      'APAGAR',
    );
    expect(res.results.find((r) => r.package === 'com.android.systemui')!.ok).toBe(false);
    expect(res.results.find((r) => r.package === 'com.miui.analytics')!.ok).toBe(true);
  });

  it('audit captures executed actions', async () => {
    const b = new MockBridge();
    await b.packageAction('23021RAA2Y', 'com.miui.analytics', 'disable', 0);
    const audit = await b.getAudit();
    expect(audit.length).toBeGreaterThan(0);
    expect(audit[0].action).toBe('disable');
    expect(audit[0].undo).toMatchObject({ action: 'enable_package' });
  });
});

describe('MockBridge: settings round-trip', () => {
  it('saves and returns settings', async () => {
    const b = new MockBridge();
    const s = await b.getSettings();
    s.logcatMaxLines = 7000;
    const saved = await b.saveSettings(s);
    expect(saved.logcatMaxLines).toBe(7000);
    const again = await b.getSettings();
    expect(again.logcatMaxLines).toBe(7000);
  });

  it('describes operations with quoted args (injection preview)', async () => {
    const b = new MockBridge();
    const desc = await b.describeOperation(
      { op: 'list_dir', path: '/sdcard/Download' },
      '23021RAA2Y',
    );
    expect(desc).toContain("adb -s 23021RAA2Y shell ls -la '/sdcard/Download'");
  });
});

it('stopping one logcat session does not stop another', async () => {
  vi.useFakeTimers();
  const b = new MockBridge();
  try {
    const first = b.logcatStart('23021RAA2Y');
    await vi.advanceTimersByTimeAsync(150);
    const id1 = await first;
    const second = b.logcatStart('23021RAA2Y');
    await vi.advanceTimersByTimeAsync(150);
    const id2 = await second;
    await b.logcatStop(id1);
    const receive = vi.fn();
    const off = b.on('logcat-line', receive);
    await vi.advanceTimersByTimeAsync(400);
    expect(receive).toHaveBeenCalledWith(expect.objectContaining({ session: id2 }));
    expect(receive).not.toHaveBeenCalledWith(expect.objectContaining({ session: id1 }));
    off();
    await b.logcatStop(id2);
  } finally { vi.clearAllTimers(); vi.useRealTimers(); }
});
