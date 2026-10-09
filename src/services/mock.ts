// Mock backend for demo mode (no Tauri) and UI tests.
// Simulates a realistic device farm: a Redmi Note 12 (USB), a Pixel over
// Wi-Fi, an emulator, one unauthorized and one offline device.

import type {
  AppInfo,
  AppPaths,
  AuditEntry,
  BackendEvent,
  BatchResult,
  BatteryInfo,
  DebloatProfile,
  Device,
  DeviceInfo,
  DeviceOperation,
  DiagnosticReport,
  DiagnosticReportExport,
  DiskUsage,
  FastbootDevice,
  FastbootOperation,
  FileEntry,
  HistoryEntry,
  NetworkInfo,
  OpResult,
  PackageMeta,
  PackageRow,
  PackageRisk,
  ReportPrivacy,
  RiskLevel,
  ScrcpyOptions,
  ScrcpyStatus,
  ScreenshotResult,
  Settings,
  ToolName,
  ToolStatus,
} from '../types';
import type { Bridge, Unsubscribe } from './bridge';
import { APP } from '../config/app';

const delay = (ms = 120) => new Promise((r) => setTimeout(r, ms));

/** Local-time stamp in the app's canonical format: YYYY-MM-DD-HHMMSS. */
function timestamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

const DEVICES: Device[] = [
  {
    serial: '23021RAA2Y',
    state: 'connected',
    model: 'Redmi Note 12',
    product: 'speser',
    device: 'speser',
    connection: 'usb',
    isEmulator: false,
  },
  {
    serial: '192.168.1.50:5555',
    state: 'connected',
    model: 'Pixel 6',
    product: 'lynx',
    device: 'lynx',
    connection: 'wifi',
    isEmulator: false,
  },
  {
    serial: 'emulator-5554',
    state: 'connected',
    model: 'sdk_gphone64_arm64',
    product: 'sdk_gphone64_arm64',
    device: 'emu64xa',
    connection: 'emulator',
    isEmulator: true,
  },
  {
    serial: 'A1B2C3D4E5',
    state: 'unauthorized',
    model: null,
    product: null,
    device: null,
    connection: 'usb',
    isEmulator: false,
  },
  {
    serial: 'F6OFF999',
    state: 'offline',
    model: 'Galaxy S20 FE',
    product: 'r11',
    device: 'r11',
    connection: 'usb',
    isEmulator: false,
  },
];

const INFO: Record<string, DeviceInfo> = {
  '23021RAA2Y': {
    serial: '23021RAA2Y',
    model: 'Redmi Note 12',
    manufacturer: 'Xiaomi',
    brand: 'Xiaomi',
    androidVersion: '15',
    sdkVersion: '35',
    securityPatch: '2026-08-01',
    buildId: 'SP1A.210812.016',
    buildDisplay: 'OS2.0.1.0.UMAUXCN',
    kernel: '5.4.97-android12-9',
    cpuAbis: 'arm64-v8a,armeabi-v7a,armeabi',
    boardPlatform: 'taro',
    hardware: 'qcom',
    product: 'speser',
    device: 'speser',
    totalRamMb: 7680,
  },
  '192.168.1.50:5555': {
    serial: '192.168.1.50:5555',
    model: 'Pixel 6',
    manufacturer: 'Google',
    brand: 'google',
    androidVersion: '14',
    sdkVersion: '34',
    securityPatch: '2026-07-05',
    buildId: 'UP1A.231005.007',
    buildDisplay: 'BP4A.250505.006',
    kernel: '5.10.198-android13-4',
    cpuAbis: 'arm64-v8a',
    boardPlatform: 'gs101',
    hardware: 'lynx',
    product: 'lynx',
    device: 'lynx',
    totalRamMb: 11264,
  },
  'emulator-5554': {
    serial: 'emulator-5554',
    model: 'sdk_gphone64_arm64',
    manufacturer: 'Google',
    brand: 'google',
    androidVersion: '15',
    sdkVersion: '35',
    securityPatch: '2026-06-01',
    buildId: 'AV1A.240920.001',
    buildDisplay: 'emulator-5554',
    kernel: '5.10.148-android13-8',
    cpuAbis: 'arm64-v8a',
    boardPlatform: 'goldfish',
    hardware: 'ranchu',
    product: 'sdk_gphone64_arm64',
    device: 'emu64xa',
    totalRamMb: 4096,
  },
};

const PACKAGES: PackageRow[] = [
  { name: 'com.android.chrome', version: '138.0.7204', versionCode: 7204, uid: 'u0_a281', path: '/data/app/com.android.chrome', isSystem: false, isDisabled: false },
  { name: 'com.miui.msa.global', version: '9.2.4', versionCode: 924, uid: 'u0_a112', path: '/system/priv-app/MsaGlobal', isSystem: true, isDisabled: false },
  { name: 'com.miui.analytics', version: '7.9.0', versionCode: 790, uid: 'u0_a113', path: '/system/app/MiuiAnalytics', isSystem: true, isDisabled: false },
  { name: 'com.lbe.xiaomi', version: '3.3.0', versionCode: 330, uid: 'u0_a114', path: '/system/app/LBE', isSystem: true, isDisabled: false },
  { name: 'com.android.systemui', version: '15.0', versionCode: 150000, uid: 'u0_a10', path: '/system/priv-app/SystemUI', isSystem: true, isDisabled: false },
  { name: 'com.android.settings', version: '15.0', versionCode: 150000, uid: 'u0_a21', path: '/system/priv-app/Settings', isSystem: true, isDisabled: false },
  { name: 'com.google.android.gms', version: '25.11.19', versionCode: 25111900, uid: 'u0_a190', path: '/system/app/GmsCore', isSystem: true, isDisabled: false },
  { name: 'org.telegram.messenger', version: '11.5.1', versionCode: 5100, uid: 'u0_a290', path: '/data/app/org.telegram.messenger', isSystem: false, isDisabled: false },
  { name: 'com.termux', version: '0.118.1', versionCode: 1181, uid: 'u0_a295', path: '/data/app/com.termux', isSystem: false, isDisabled: false },
  { name: 'com.miui.videoplayer', version: '12.0.21', versionCode: 12021, uid: 'u0_a120', path: '/system/app/MIUIVideo', isSystem: true, isDisabled: true },
  { name: 'com.example.promo', version: '1.0.0', versionCode: 1, uid: 'u0_a300', path: '/data/app/com.example.promo', isSystem: false, isDisabled: false },
];

const FILE_TREE: Record<string, FileEntry[]> = {
  '/sdcard': [
    { name: 'DCIM', isDir: true, isLink: false, size: 4096, modified: 'Aug 12 10:00', permissions: 'drwxrwx---' },
    { name: 'Download', isDir: true, isLink: false, size: 4096, modified: 'Sep 01 18:22', permissions: 'drwxrwx---' },
    { name: 'Movies', isDir: true, isLink: false, size: 4096, modified: 'Jul 30 09:15', permissions: 'drwxrwx---' },
    { name: 'Music', isDir: true, isLink: false, size: 4096, modified: 'Jul 30 09:15', permissions: 'drwxrwx---' },
    { name: 'Pictures', isDir: true, isLink: false, size: 4096, modified: 'Aug 20 21:41', permissions: 'drwxrwx---' },
    { name: 'Android', isDir: true, isLink: false, size: 4096, modified: 'Jun 02 08:00', permissions: 'drwxrwx---' },
    { name: 'notas.txt', isDir: false, isLink: false, size: 2048, modified: 'Sep 10 14:12', permissions: '-rw-rw----' },
  ],
  '/sdcard/DCIM': [
    { name: 'Camera', isDir: true, isLink: false, size: 4096, modified: 'Sep 02 19:03', permissions: 'drwxrwx---' },
    { name: 'Screenshots', isDir: true, isLink: false, size: 4096, modified: 'Aug 25 11:30', permissions: 'drwxrwx---' },
  ],
  '/sdcard/Download': [
    { name: 'app-debug.apk', isDir: false, isLink: false, size: 38_400_000, modified: 'Sep 15 10:10', permissions: '-rw-rw----' },
    { name: 'arquivo.zip', isDir: false, isLink: false, size: 1_700_000_000, modified: 'Sep 18 16:44', permissions: '-rw-rw----' },
    { name: 'documento.pdf', isDir: false, isLink: false, size: 812_000, modified: 'Aug 30 09:00', permissions: '-rw-rw----' },
  ],
  '/sdcard/Pictures': [
    { name: 'Screenshot_20260825_113000.png', isDir: false, isLink: false, size: 1_240_000, modified: 'Aug 25 11:30', permissions: '-rw-rw----' },
  ],
};

const SHELL_CANNED: [RegExp, string][] = [
  [/^getprop ro\.product\.model$/, 'Redmi Note 12'],
  [/^getprop ro\.build\.version\.release$/, '15'],
  [/^getprop ro\.build\.version\.sdk$/, '35'],
  [/^getprop$/, 'ro.product.model: Redmi Note 12\nro.product.manufacturer: Xiaomi\nro.build.version.release: 15\nro.build.version.sdk: 35\nro.build.id: SP1A.210812.016'],
  [/^uname -r$/, '5.4.97-android12-9'],
  [/^ls( -\w+)* \/?$/, 'total 3\ndrwxrwx--- 2 u0 u0 4096 2026-08-12 10:00 DCIM\ndrwxrwx--- 2 u0 u0 4096 2026-09-01 18:22 Download\n-rw-rw---- 1 u0 u0 2048 2026-09-10 14:12 notas.txt'],
  [/^echo (.*)$/, '$1'],
  [/^whoami$/, 'shell'],
  [/^pm list packages -3$/, 'package:com.android.chrome\npackage:org.telegram.messenger\npackage:com.termux\npackage:com.example.promo'],
];

const RISK_TABLE: Record<string, RiskLevel> = {
  'com.miui.msa.global': 'SAFE',
  'com.miui.analytics': 'SAFE',
  'com.lbe.xiaomi': 'SAFE',
  'com.example.promo': 'SAFE',
  'com.miui.videoplayer': 'LOW_RISK',
  'com.android.systemui': 'CRITICAL',
  'com.android.settings': 'CRITICAL',
  'com.google.android.gms': 'CAUTION',
};
function riskOf(pkg: string): RiskLevel {
  if (RISK_TABLE[pkg]) return RISK_TABLE[pkg];
  if (pkg.startsWith('com.miui.')) return 'LOW_RISK';
  if (pkg.startsWith('com.android.')) return 'DANGEROUS';
  return 'UNKNOWN';
}

const PROFILES: DebloatProfile[] = [
  { id: 'conservative', name: 'Conservador', maxRisk: 'SAFE', description: '' },
  { id: 'balanced', name: 'Equilibrado', maxRisk: 'LOW_RISK', description: '' },
  { id: 'minimal', name: 'Minimal', maxRisk: 'LOW_RISK', description: '' },
  { id: 'advanced', name: 'Avançado', maxRisk: 'CAUTION', description: '' },
];

type Listener = (payload: unknown) => void;

export class MockBridge implements Bridge {
  readonly isDemo = true;
  private listeners = new Map<BackendEvent, Set<Listener>>();
  private timers = new Set<ReturnType<typeof setInterval>>();
  private scrcpyRunning = false;
  private scrcpyRecording = false;
  private scrcpyPid = 0;
  private shellCounter = 0;
  private logcatCounter = 0;
  private logcatTimers = new Map<string, ReturnType<typeof setInterval>>();
  private transferCounter = 0;
  private audit: AuditEntry[] = [
    {
      ts: Date.now() - 3_600_000,
      device: '23021RAA2Y',
      action: 'disable_package',
      command: "adb -s 23021RAA2Y shell pm disable-user --user 0 com.example.test",
      result: 'ok',
      undo: { action: 'enable_package', package: 'com.example.test' },
    },
  ];
  private history: HistoryEntry[] = [
    {
      serial: '23021RAA2Y',
      model: 'Redmi Note 12',
      lastState: 'connected',
      lastSeenMs: Date.now() - 3_600_000,
      connection: 'usb',
      alias: 'Redmi de QA',
      tags: ['qa'],
      favorite: true,
    },
  ];
  private settings: Settings = {
    theme: 'system',
    language: 'auto',
    performanceMode: 'normal',
    adbPath: null,
    scrcpyPath: null,
    fastbootPath: null,
    screenshotDir: null,
    recordingDir: null,
    downloadDir: null,
    autoRefreshSecs: null,
    logcatMaxLines: 5000,
    auditEnabled: true,
  };

  private emit(event: BackendEvent, payload: unknown): void {
    this.listeners.get(event)?.forEach((cb) => cb(payload));
  }

  on(event: BackendEvent, cb: Listener): Unsubscribe {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(cb);
    return () => this.listeners.get(event)?.delete(cb);
  }

  // ---- tools ----------------------------------------------------------

  async detectTools(): Promise<ToolStatus[]> {
    await delay(80);
    return [
      { name: 'adb', found: true, path: '/usr/bin/adb', version: 'Android Debug Bridge version 1.0.41 (simulado)', source: 'path' },
      { name: 'scrcpy', found: true, path: '/usr/local/bin/scrcpy', version: 'scrcpy 2.7 (simulado)', source: 'path' },
      { name: 'fastboot', found: false, path: null, version: null, source: null },
    ];
  }

  async setToolPath(tool: ToolName, path: string | null): Promise<ToolStatus[]> {
    await delay(80);
    const base = await this.detectTools();
    return base.map((t) =>
      t.name === tool
        ? { ...t, found: !!path, path, source: path ? 'manual' : t.source, version: path ? 'versão local' : null }
        : t,
    );
  }

  async checkToolPath(path: string): Promise<boolean> {
    await delay(30);
    return path.length > 3;
  }

  // ---- devices ---------------------------------------------------------

  async listDevices(): Promise<Device[]> {
    await delay(150);
    this.history = [
      ...DEVICES.map((d) => ({
        serial: d.serial,
        model: d.model,
        lastState: d.state,
        lastSeenMs: Date.now(),
        connection: d.connection,
        alias: this.history.find((h) => h.serial === d.serial)?.alias ?? null,
        tags: this.history.find((h) => h.serial === d.serial)?.tags ?? [],
        favorite: this.history.find((h) => h.serial === d.serial)?.favorite ?? false,
      })),
      ...this.history.filter((h) => !DEVICES.some((d) => d.serial === h.serial)),
    ];
    return DEVICES.map((d) => ({ ...d }));
  }

  async getDeviceInfo(serial: string): Promise<DeviceInfo> {
    await delay(150);
    this.ensureDevice(serial); // unauthorized/offline surface the proper code
    const info = INFO[serial];
    if (!info) throw Object.assign(new Error('no device'), { code: 'NO_DEVICE', details: serial });
    return { ...info };
  }

  async getBattery(serial: string): Promise<BatteryInfo> {
    await delay(100);
    this.ensureDevice(serial);
    return {
      level: 73,
      temperature: 310,
      temperatureC: 31,
      status: '2',
      health: '2',
      technology: 'Li-ion',
      voltageMv: 4101,
      plugged: '1',
    };
  }

  async getStorage(serial: string): Promise<DiskUsage> {
    await delay(100);
    this.ensureDevice(serial);
    return { totalMb: 245_760, usedMb: 173_210, availMb: 72_550, path: '/sdcard' };
  }

  async getNetworkInfo(serial: string): Promise<NetworkInfo> {
    await delay(100);
    this.ensureDevice(serial);
    return {
      interfaces: [{ name: 'wlan0', ip: '192.168.1.50', prefix: 24 }],
      gateway: '192.168.1.1',
      dns: ['192.168.1.1'],
      mac: 'aa:bb:cc:11:22:33',
    };
  }

  async getDeviceProps(serial: string): Promise<Record<string, string>> {
    await delay(200);
    this.ensureDevice(serial);
    const info = INFO[serial];
    const props: Record<string, string> = {
      'ro.product.model': info.model ?? 'unknown',
      'ro.product.manufacturer': info.manufacturer ?? 'unknown',
      'ro.build.version.release': info.androidVersion ?? 'unknown',
      'ro.build.version.sdk': info.sdkVersion ?? 'unknown',
      'ro.build.id': info.buildId ?? 'unknown',
      'ro.build.display.id': info.buildDisplay ?? 'unknown',
      'ro.build.version.security_patch': info.securityPatch ?? 'unknown',
      'ro.board.platform': info.boardPlatform ?? 'unknown',
      'ro.hardware': info.hardware ?? 'unknown',
      'ro.product.cpu.abilist': info.cpuAbis ?? 'unknown',
    };
    return props;
  }

  async collectDiagnosticReport(serial: string, privacy: ReportPrivacy): Promise<DiagnosticReport> {
    await delay(180);
    this.ensureDevice(serial);
    const [info, battery, storage, network, tools] = await Promise.all([
      this.getDeviceInfo(serial),
      this.getBattery(serial),
      this.getStorage(serial),
      this.getNetworkInfo(serial),
      this.detectTools(),
    ]);
    return {
      schemaVersion: 1,
      generatedAt: new Date().toISOString(),
      appVersion: APP.version,
      device: {
        serial: privacy.includeSerial ? serial : `…${serial.slice(-4)}`,
        state: 'connected',
      },
      tools: tools.map(({ name, found, version }) => ({ name, found, version })),
      sections: {
        connection: { status: 'ok', data: { state: 'connected', connection: 'usb', isEmulator: false }, error: null },
        properties: { status: 'ok', data: info, error: null },
        memory: { status: 'ok', data: { totalRamMb: info.totalRamMb }, error: null },
        battery: { status: 'ok', data: battery, error: null },
        storage: { status: 'ok', data: storage, error: null },
        network: serial === 'emulator-5554'
          ? { status: 'unavailable', data: null, error: { code: 'DEVICE_UNREACHABLE', details: 'network data unavailable in emulator demo' } }
          : {
              status: 'ok',
              data: privacy.includeNetwork ? network : { interfaces: [], gateway: '[redacted]', dns: [], mac: '[redacted]' },
              error: null,
            },
      },
      limitations: [],
    };
  }

  async exportDiagnosticReport(report: DiagnosticReport): Promise<DiagnosticReportExport> {
    await delay(120);
    return {
      report,
      jsonPath: `~/Downloads/Zittodb/zittodb-diagnostic-${timestamp()}.json`,
      markdownPath: `~/Downloads/Zittodb/zittodb-diagnostic-${timestamp()}.md`,
    };
  }

  async adbConnect(host: string, port: number): Promise<string> {
    await delay(400);
    if (!host || !port) return 'failed to connect to 0.0.0.0:0';
    return `connected to ${host}:${port}`;
  }

  async adbDisconnect(host: string, port: number): Promise<string> {
    await delay(100);
    return `disconnected ${host}:${port}`;
  }

  async adbServerVersion(): Promise<string> {
    await delay(80);
    return 'Android Debug Bridge version 1.0.41 (simulado)';
  }

  async adbServerStart(): Promise<string> {
    await delay(120);
    return '* daemon started successfully (simulado)';
  }

  async adbServerRestart(confirmation: string): Promise<string> {
    await delay(160);
    if (confirmation !== 'REINICIAR_ADB') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type REINICIAR_ADB to confirm' });
    }
    return '* daemon restarted successfully (simulado)';
  }

  async rebootDevice(
    _serial: string | null,
    _target: 'system' | 'bootloader' | 'recovery' | 'sideload',
    confirmation: string,
  ): Promise<OpResult> {
    await delay(300);
    if (confirmation !== 'REINICIAR') {
      throw Object.assign(new Error('confirm'), {
        code: 'CONFIRMATION_REQUIRED',
        details: 'type REINICIAR to confirm',
      });
    }
    return { ok: true, stdout: 'rebooting…', stderr: '', code: null };
  }

  private ensureDevice(serial: string): void {
    const d = DEVICES.find((x) => x.serial === serial);
    if (!d) throw Object.assign(new Error('no device'), { code: 'NO_DEVICE', details: serial });
    if (d.state === 'unauthorized') {
      throw Object.assign(new Error('unauthorized'), {
        code: 'DEVICE_UNAUTHORIZED',
        details: `device ${serial} unauthorized`,
      });
    }
    if (d.state !== 'connected') {
      throw Object.assign(new Error('offline'), { code: 'DEVICE_OFFLINE', details: serial });
    }
  }

  // ---- media ------------------------------------------------------------

  async screenshot(_serial: string | null, force: boolean): Promise<ScreenshotResult> {
    await delay(500);
    const name = `screenshot-${timestamp()}${force ? '-1' : ''}.png`;
    return { path: `~/Pictures/Zittodb/${name}`, sizeBytes: 842_113 };
  }

  async pickPath(kind: 'apk' | 'file' | 'directory'): Promise<string | null> {
    await delay(50);
    if (kind === 'directory') return '~/Downloads/Zittodb';
    if (kind === 'apk') return '~/Downloads/app-debug.apk';
    return '~/Downloads/arquivo.zip';
  }

  async openPath(_path: string): Promise<void> {
    await delay(20);
  }

  async copyImageToClipboard(_path: string): Promise<void> {
    await delay(100);
    // Browser demo: pretend success (no X11/Wayland here).
  }

  // ---- scrcpy --------------------------------------------------------------

  async scrcpyStart(serial: string, options: ScrcpyOptions): Promise<ScrcpyStatus> {
    await delay(250);
    this.ensureDevice(serial);
    if (this.scrcpyRunning) {
      throw Object.assign(new Error('already'), { code: 'ALREADY_RUNNING', details: 'scrcpy running' });
    }
    this.scrcpyRunning = true;
    this.scrcpyRecording = !!options.recordPath;
    this.scrcpyPid = 42000 + Math.floor(Math.random() * 9000);
    const lines = [
      'INFO: adb device found',
      `INFO: using device ${serial}`,
      `INFO: max size ${options.maxSize ?? 'native'}, fps ${options.maxFps ?? 'native'}`,
      'INFO: recording to ' + (options.recordPath ?? '(off)'),
    ];
    for (const line of lines) {
      await delay(60);
      this.emit('scrcpy-log', { serial, line: line + '\n' });
    }
    return { running: true, pid: this.scrcpyPid, recording: this.scrcpyRecording };
  }

  async scrcpyStop(): Promise<ScrcpyStatus> {
    await delay(120);
    this.scrcpyRunning = false;
    this.scrcpyRecording = false;
    this.emit('scrcpy-log', { serial: '', line: 'INFO: scrcpy ended\n' });
    return { running: false, pid: null, recording: false };
  }

  async scrcpyStatus(): Promise<ScrcpyStatus> {
    return { running: this.scrcpyRunning, pid: this.scrcpyRunning ? this.scrcpyPid : null, recording: this.scrcpyRecording };
  }

  async recordingDir(): Promise<string> {
    return '~/Videos/Zittodb';
  }

  async recordingFilename(): Promise<string> {
    return `~/Videos/Zittodb/recording-${timestamp()}.mp4`;
  }

  // ---- shell ---------------------------------------------------------------

  async shellOpen(serial: string): Promise<{ id: string; serial: string }> {
    await delay(200);
    this.ensureDevice(serial);
    const id = `sh${++this.shellCounter}`;
    this.emit('shell-output', {
      session: id,
      line: `ADB SHELL — Dispositivo: ${serial}\n`,
    });
    return { id, serial };
  }

  async shellWrite(id: string, data: string): Promise<void> {
    const lines = data.split('\n').filter(Boolean);
    for (const raw of lines) {
      const cmd = raw.trim();
      if (!cmd) continue;
      await delay(90);
      let out = '';
      for (const [re, canned] of SHELL_CANNED) {
        const m = cmd.match(re);
        if (m) {
          out = canned.replace(/\$1/g, m[1] ?? '').replace(/\$2/g, m[2] ?? '') + '\n';
          break;
        }
      }
      if (!out) {
        out = `/system/bin/sh: ${cmd.split(' ')[0]}: inaccessible or not found\n`;
      }
      this.emit('shell-output', { session: id, line: `$ ${cmd}\n${out}` });
    }
  }

  async shellClose(id: string): Promise<void> {
    this.emit('shell-output', { session: id, line: '[sessão encerrada]\n' });
  }

  async shellCloseAll(): Promise<number> {
    return 0;
  }

  // ---- packages ---------------------------------------------------------------

  async listPackages(serial: string): Promise<PackageRow[]> {
    await delay(600);
    this.ensureDevice(serial);
    return PACKAGES.map((p) => ({ ...p }));
  }

  async packageInfo(serial: string, pkg: string): Promise<PackageMeta> {
    await delay(200);
    this.ensureDevice(serial);
    const p = PACKAGES.find((x) => x.name === pkg);
    if (!p) throw Object.assign(new Error('no pkg'), { code: 'PROCESS_FAILED', details: `package not found: ${pkg}` });
    return {
      name: p.name,
      versionName: p.version,
      versionCode: p.versionCode,
      uid: p.uid,
      codePath: p.path,
      firstInstall: 1_700_000_000_000,
      lastUpdate: 1_750_000_000_000,
    };
  }

  async packageAction(
    serial: string | null,
    pkg: string,
    action: 'open' | 'enable' | 'disable' | 'uninstall_for_user' | 'clear_data' | 'reinstall',
    _user: number,
    confirmation?: string,
  ): Promise<OpResult> {
    await delay(400);
    if (serial) this.ensureDevice(serial);
    const destructive: Record<string, string> = {
      uninstall_for_user: 'REMOVER',
      clear_data: 'APAGAR',
    };
    const word = destructive[action];
    if (word && confirmation !== word) {
      throw Object.assign(new Error('confirm'), {
        code: 'CONFIRMATION_REQUIRED',
        details: `type ${word} to confirm`,
      });
    }
    this.audit.unshift({
      ts: Date.now(),
      device: serial ?? null,
      action: action.replace('_', '_'),
      command: `adb -s ${serial ?? '?'} shell <${action}> ${pkg}`,
      result: 'ok',
      undo:
        action === 'disable'
          ? { action: 'enable_package', package: pkg }
          : action === 'uninstall_for_user'
            ? { action: 'reinstall_existing', package: pkg }
            : null,
    });
    return { ok: true, stdout: action === 'open' ? 'Events injected' : '', stderr: '', code: null };
  }

  async installApk(serial: string, local: string): Promise<OpResult> {
    await delay(1500);
    this.ensureDevice(serial);
    if (!local.toLowerCase().endsWith('.apk')) {
      throw Object.assign(new Error('not an apk'), {
        code: 'INVALID_ARGUMENT',
        details: `expected .apk file, got: ${local}`,
      });
    }
    return { ok: true, stdout: 'Performing Streamed Install\nSuccess', stderr: '', code: null };
  }

  async packageExtract(serial: string, pkg: string, _destDir?: string): Promise<string> {
    await delay(1500);
    this.ensureDevice(serial);
    const row = PACKAGES.find((p) => p.name === pkg);
    return `~/Downloads/Zittodb/${(row?.path ?? pkg).split('/').pop() ?? pkg}.apk`;
  }

  // ---- files -------------------------------------------------------------------

  async fileList(serial: string, path: string): Promise<FileEntry[]> {
    await delay(200);
    this.ensureDevice(serial);
    if (!path.startsWith('/')) throw Object.assign(new Error('bad path'), { code: 'INVALID_PATH', details: path });
    return FILE_TREE[path] ?? [];
  }

  async fileMkdir(_serial: string, path: string): Promise<OpResult> {
    await delay(150);
    if (!path.startsWith('/sdcard/')) throw Object.assign(new Error('bad path'), { code: 'INVALID_PATH', details: path });
    return { ok: true, stdout: '', stderr: '', code: null };
  }

  async fileRename(_serial: string, _from: string, _to: string): Promise<OpResult> {
    await delay(150);
    return { ok: true, stdout: '', stderr: '', code: null };
  }

  async fileDelete(_serial: string, _path: string, confirmation: string): Promise<OpResult> {
    await delay(200);
    if (confirmation !== 'APAGAR') {
      throw Object.assign(new Error('confirm'), {
        code: 'CONFIRMATION_REQUIRED',
        details: 'type APAGAR to confirm',
      });
    }
    return { ok: true, stdout: '', stderr: '', code: null };
  }

  async filePush(serial: string, local: string, remote?: string): Promise<string> {
    await delay(100);
    this.ensureDevice(serial);
    const id = `tx${++this.transferCounter}`;
    const target = remote ?? `/sdcard/${local.split('/').pop()}`;
    const start = Date.now();
    const t = setInterval(() => {
      const elapsed = Date.now() - start;
      if (elapsed > 2500) {
        clearInterval(t);
        this.timers.delete(t);
        this.emit('file-progress', {
          id,
          status: 'done',
          doneBytes: 1_700_000_000,
          total: 1_700_000_000,
          message: `1700000000 bytes pushed to ${target} (simulado)`,
        });
      } else {
        // adb push: no native progress — demo shows indeterminate via running events
        this.emit('file-progress', { id, status: 'running', doneBytes: 0, total: 1_700_000_000, message: '' });
      }
    }, 250);
    this.timers.add(t);
    return id;
  }

  async filePull(serial: string, remote: string, _localDir?: string): Promise<string> {
    await delay(100);
    this.ensureDevice(serial);
    const id = `tx${++this.transferCounter}`;
    const total = 1_700_000_000;
    let done = 0;
    const t = setInterval(() => {
      done += 170_000_000;
      if (done >= total) {
        clearInterval(t);
        this.timers.delete(t);
        this.emit('file-progress', {
          id,
          status: 'done',
          doneBytes: total,
          total,
          message: `1700000000 bytes pulled from ${remote} (simulado)`,
        });
      } else {
        this.emit('file-progress', { id, status: 'running', doneBytes: done, total, message: '' });
      }
    }, 300);
    this.timers.add(t);
    return id;
  }

  async transferCancel(id: string): Promise<void> {
    for (const t of this.timers) clearInterval(t);
    this.timers.clear();
    this.emit('file-progress', { id, status: 'cancelled', doneBytes: 0, total: null, message: 'cancelled' });
  }

  // ---- logs ---------------------------------------------------------------------

  async logcatStart(serial: string, spec?: string): Promise<string> {
    await delay(150);
    this.ensureDevice(serial);
    const id = `lc${++this.logcatCounter}`;
    let n = 0;
    const tags = ['ActivityManager', 'Camera', 'USB', 'Wifi', 'AudioFlinger'];
    const msgs = [
      'START u0 {act=android.intent.action.MAIN}',
      'Display is attached',
      'Device added: /dev/bus/usb/001/004',
      'Supplicant state change: ASSOCIATED -> ASSOCIATED',
      'audio_hw primary: open() called',
    ];
    const t = setInterval(() => {
      n += 1;
      const tag = spec?.split(':')?.[0] ?? tags[n % tags.length];
      const level = ['I', 'D', 'W', 'E', 'I'][n % 5];
      const time = new Date().toLocaleTimeString('pt-BR', { hour12: false });
      const frac = String((n * 137) % 1000).padStart(3, '0');
      this.emit('logcat-line', {
        session: id,
        line: `${time}.${frac}  1234  5678 ${level} ${tag}: ${msgs[n % msgs.length]}\n`,
      });
    }, 400);
    this.logcatTimers.set(id, t);
    return id;
  }

  async logcatStop(id: string): Promise<void> {
    const timer = this.logcatTimers.get(id);
    if (timer) clearInterval(timer);
    this.logcatTimers.delete(id);
    this.emit('logcat-line', { session: id, line: '[logcat encerrado]\n' });
  }

  async saveLogFile(_path: string, _content: string): Promise<void> {
    await delay(80);
  }

  // ---- operations / builder / debloat -----------------------------------------------

  async describeOperation(op: DeviceOperation, serial: string | null): Promise<string> {
    await delay(20);
    const s = serial ?? '<serial>';
    const adb = '/usr/bin/adb';
    const base = op.op;
    const arg = (v: string) => `'${v}'`;
    switch (base) {
      case 'get_props':
        return `${adb} -s ${s} shell getprop`;
      case 'get_kernel':
        return `${adb} -s ${s} shell uname -r`;
      case 'get_meminfo':
        return `${adb} -s ${s} shell cat /proc/meminfo`;
      case 'get_disk_usage':
        return `${adb} -s ${s} shell df -m ${arg(op.path)}`;
      case 'get_battery':
        return `${adb} -s ${s} shell dumpsys battery`;
      case 'get_network':
        return `${adb} -s ${s} shell ip addr show`;
      case 'get_resolv':
        return `${adb} -s ${s} shell cat /etc/resolv.conf`;
      case 'get_mac':
        return `${adb} -s ${s} shell cat /sys/class/net/${op.iface}/address`;
      case 'screenshot':
        return `${adb} -s ${s} exec-out screencap -p`;
      case 'list_packages':
        return `${adb} -s ${s} shell pm list packages${op.thirdParty ? ' -3' : ''}${op.disabled ? ' -d' : ''}`;
      case 'package_path':
        return `${adb} -s ${s} shell pm path ${arg(op.pkg)}`;
      case 'package_dump':
        return `${adb} -s ${s} shell dumpsys package ${arg(op.pkg)}`;
      case 'open_package':
        return `${adb} -s ${s} shell monkey -p ${arg(op.pkg)} -c android.intent.category.LAUNCHER 1`;
      case 'enable_package':
        return `${adb} -s ${s} shell pm enable ${arg(op.pkg)}`;
      case 'disable_package':
        return `${adb} -s ${s} shell pm disable-user --user ${op.user} ${arg(op.pkg)}`;
      case 'uninstall_for_user':
        return `${adb} -s ${s} shell pm uninstall -k --user ${op.user} ${arg(op.pkg)}`;
      case 'reinstall_existing':
        return `${adb} -s ${s} shell pm install-existing ${arg(op.pkg)}`;
      case 'clear_package_data':
        return `${adb} -s ${s} shell pm clear ${arg(op.pkg)}`;
      case 'install_apk':
        return `${adb} -s ${s} install -r ${op.local}`;
      case 'list_dir':
        return `${adb} -s ${s} shell ls -la ${arg(op.path)}`;
      case 'mkdir':
        return `${adb} -s ${s} shell mkdir -p ${arg(op.path)}`;
      case 'rename':
        return `${adb} -s ${s} shell mv ${arg(op.from)} ${arg(op.to)}`;
      case 'delete':
        return `${adb} -s ${s} shell rm -rf ${arg(op.path)}`;
      case 'push':
        return `${adb} -s ${s} push ${op.local} ${op.remote}`;
      case 'pull':
        return `${adb} -s ${s} pull ${op.remote} ${op.local}`;
      case 'reboot':
        return `${adb} -s ${s} reboot ${op.target}`;
      case 'connect':
        return `${adb} connect ${op.host}:${op.port}`;
      case 'disconnect':
        return `${adb} disconnect ${op.host}:${op.port}`;
      default:
        return `${adb} (op desconhecida)`;
    }
  }

  async executeOperation(
    serial: string | null,
    op: DeviceOperation,
    confirmation?: string,
  ): Promise<OpResult> {
    await delay(300);
    if (serial) this.ensureDevice(serial);
    if (op.op === 'delete' && confirmation !== 'APAGAR') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type APAGAR' });
    }
    if (op.op === 'uninstall_for_user' && confirmation !== 'REMOVER') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type REMOVER' });
    }
    if (op.op === 'reboot' && confirmation !== 'REINICIAR') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type REINICIAR' });
    }
    return { ok: true, stdout: '', stderr: '', code: null };
  }

  async executeBatch(
    serial: string,
    op: DeviceOperation,
    packages: string[],
    confirmation?: string,
  ): Promise<BatchResult> {
    this.ensureDevice(serial);
    const results = [];
    for (let i = 0; i < packages.length; i++) {
      await delay(250);
      const pkg = packages[i];
      const fails = riskOf(pkg) === 'CRITICAL';
      results.push({
        index: i,
        package: pkg,
        ok: !fails,
        code: fails ? 'OPERATION_REJECTED' : null,
        message: fails ? 'pacote crítico protegido (demo)' : '',
      });
      this.audit.unshift({
        ts: Date.now(),
        device: serial,
        action: op.op,
        command: `${op.op} ${pkg}`,
        result: fails ? 'error:OPERATION_REJECTED' : 'ok',
        undo:
          op.op === 'disable_package'
            ? { action: 'enable_package', package: pkg }
            : op.op === 'uninstall_for_user'
              ? { action: 'reinstall_existing', package: pkg }
              : null,
      });
    }
    void confirmation;
    return { serial, results };
  }

  async classifyPackages(packages: string[]): Promise<PackageRisk[]> {
    await delay(40);
    return packages.map((p) => ({ package: p, risk: riskOf(p) }));
  }

  async debloatProfiles(): Promise<DebloatProfile[]> {
    return PROFILES.map((p) => ({ ...p }));
  }

  // ---- fastboot --------------------------------------------------------------------

  async fastbootDevices(): Promise<FastbootDevice[]> {
    await delay(300);
    return [{ serial: 'FAKEFB01', state: 'FASTBOOT' }];
  }

  async fastbootExecute(_serial: string | null, op: FastbootOperation, confirmation?: string): Promise<OpResult> {
    await delay(400);
    if (op.op === 'erase' && confirmation !== 'APAGAR') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type APAGAR' });
    }
    if (op.op === 'flash' && confirmation !== 'FLASHAR') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type FLASHAR' });
    }
    if ((op.op === 'unlock' || op.op === 'lock') && confirmation !== 'APAGAR') {
      throw Object.assign(new Error('confirm'), { code: 'CONFIRMATION_REQUIRED', details: 'type APAGAR' });
    }
    return { ok: true, stdout: 'OKAY [ 0.056s]\nfinished. total time: 0.056s', stderr: '', code: null };
  }

  // ---- settings / history / app -------------------------------------------------------

  async getSettings(): Promise<Settings> {
    await delay(30);
    return { ...this.settings };
  }

  async saveSettings(settings: Settings): Promise<Settings> {
    await delay(60);
    this.settings = { ...settings };
    return { ...this.settings };
  }

  async getAudit(limit?: number): Promise<AuditEntry[]> {
    await delay(40);
    return this.audit.slice(0, limit ?? 200).map((a) => ({ ...a }));
  }

  async clearAudit(): Promise<void> {
    this.audit = [];
  }

  async getDeviceHistory(): Promise<HistoryEntry[]> {
    await delay(40);
    return this.history.map((h) => ({ ...h }));
  }

  async clearDeviceHistory(): Promise<void> {
    this.history = [];
  }

  async updateDeviceMetadata(serial: string, alias: string | null, tags: string[], favorite: boolean): Promise<HistoryEntry[]> {
    await delay(40);
    const existing = this.history.find((h) => h.serial === serial);
    if (existing) Object.assign(existing, { alias: alias?.trim() || null, tags, favorite });
    else this.history.push({ serial, model: null, lastState: null, lastSeenMs: null, connection: null, alias: alias?.trim() || null, tags, favorite });
    return this.history.map((h) => ({ ...h, tags: [...h.tags] }));
  }

  async getAppInfo(): Promise<AppInfo> {
    return { name: 'Zittodb', version: APP.version, tauriVersion: 'demo', platform: 'web-demo' };
  }

  async getAppPaths(): Promise<AppPaths> {
    return {
      configDir: '~/.config/app.zittodb.desktop',
      dataDir: '~/.local/share/app.zittodb.desktop',
      logFile: '~/.local/share/app.zittodb.desktop/logs/zittodb.log',
      auditFile: '~/.config/app.zittodb.desktop/audit.jsonl',
    };
  }
}
