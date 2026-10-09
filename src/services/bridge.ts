// The ONLY place the UI talks to the backend (spec §42).
//
// - Inside Tauri: every call is a typed command on the Rust side; the
//   frontend NEVER executes shell (no shell plugin exists at all).
// - Browser development: same-origin Node service executes local ADB.
// - Explicit demo mode, static preview and CI use MockBridge.

import { DEFAULT_SETTINGS } from '../config/app';

import type {
  AppError,
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
  ScrcpyOptions,
  ScrcpyStatus,
  ScreenshotResult,
  Settings,
  ToolName,
  ToolStatus,
} from '../types';

export type Unsubscribe = () => void;

export interface Bridge {
  readonly isDemo: boolean;

  // tools
  detectTools(): Promise<ToolStatus[]>;
  setToolPath(tool: ToolName, path: string | null): Promise<ToolStatus[]>;
  checkToolPath(path: string): Promise<boolean>;

  // devices
  listDevices(): Promise<Device[]>;
  getDeviceInfo(serial: string): Promise<DeviceInfo>;
  getBattery(serial: string): Promise<BatteryInfo>;
  getStorage(serial: string): Promise<DiskUsage>;
  getNetworkInfo(serial: string): Promise<NetworkInfo>;
  getDeviceProps(serial: string): Promise<Record<string, string>>;
  adbConnect(host: string, port: number): Promise<string>;
  adbDisconnect(host: string, port: number): Promise<string>;
  adbServerVersion(): Promise<string>;
  adbServerStart(): Promise<string>;
  adbServerRestart(confirmation: string): Promise<string>;
  rebootDevice(
    serial: string | null,
    target: 'system' | 'bootloader' | 'recovery' | 'sideload',
    confirmation: string,
  ): Promise<OpResult>;

  // media
  screenshot(serial: string | null, force: boolean): Promise<ScreenshotResult>;
  pickPath(kind: 'apk' | 'file' | 'directory'): Promise<string | null>;
  openPath(path: string): Promise<void>;
  copyImageToClipboard(path: string): Promise<void>;

  // scrcpy
  scrcpyStart(serial: string, options: ScrcpyOptions): Promise<ScrcpyStatus>;
  scrcpyStop(): Promise<ScrcpyStatus>;
  scrcpyStatus(): Promise<ScrcpyStatus>;
  recordingDir(): Promise<string>;
  recordingFilename(): Promise<string>;

  // shell
  shellOpen(serial: string): Promise<{ id: string; serial: string }>;
  shellWrite(id: string, data: string): Promise<void>;
  shellClose(id: string): Promise<void>;
  shellCloseAll(): Promise<number>;

  // packages
  listPackages(serial: string): Promise<PackageRow[]>;
  packageInfo(serial: string, pkg: string): Promise<PackageMeta>;
  packageAction(
    serial: string | null,
    pkg: string,
    action:
      | 'open'
      | 'enable'
      | 'disable'
      | 'uninstall_for_user'
      | 'clear_data'
      | 'reinstall',
    user: number,
    confirmation?: string,
  ): Promise<OpResult>;
  packageExtract(serial: string, pkg: string, destDir?: string): Promise<string>;
  /** `adb -s S install -r <local>` (updates replace in place). */
  installApk(serial: string, local: string): Promise<OpResult>;

  // files
  fileList(serial: string, path: string): Promise<FileEntry[]>;
  fileMkdir(serial: string, path: string): Promise<OpResult>;
  fileRename(serial: string, from: string, to: string): Promise<OpResult>;
  fileDelete(serial: string, path: string, confirmation: string): Promise<OpResult>;
  filePush(serial: string, local: string, remote?: string): Promise<string>;
  filePull(serial: string, remote: string, localDir?: string): Promise<string>;
  transferCancel(id: string): Promise<void>;

  // logs
  logcatStart(serial: string, spec?: string): Promise<string>;
  logcatStop(id: string): Promise<void>;
  saveLogFile(path: string, content: string): Promise<void>;

  // operations / builder / debloat
  describeOperation(op: DeviceOperation, serial: string | null): Promise<string>;
  executeOperation(
    serial: string | null,
    op: DeviceOperation,
    confirmation?: string,
  ): Promise<OpResult>;
  executeBatch(
    serial: string,
    op: DeviceOperation,
    packages: string[],
    confirmation?: string,
  ): Promise<BatchResult>;
  classifyPackages(packages: string[]): Promise<PackageRisk[]>;
  debloatProfiles(): Promise<DebloatProfile[]>;

  // fastboot
  fastbootDevices(): Promise<FastbootDevice[]>;
  fastbootExecute(serial: string | null, op: FastbootOperation, confirmation?: string): Promise<OpResult>;

  // settings / history / app
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<Settings>;
  getAudit(limit?: number): Promise<AuditEntry[]>;
  clearAudit(): Promise<void>;
  getDeviceHistory(): Promise<HistoryEntry[]>;
  clearDeviceHistory(): Promise<void>;
  updateDeviceMetadata(serial: string, alias: string | null, tags: string[], favorite: boolean): Promise<HistoryEntry[]>;
  getAppInfo(): Promise<AppInfo>;
  getAppPaths(): Promise<AppPaths>;

  // events
  on(event: BackendEvent, cb: (payload: unknown) => void): Unsubscribe;
}

// ---------------------------------------------------------------------------

export function isTauri(): boolean {
  return (
    typeof window !== 'undefined' &&
    '__TAURI_INTERNALS__' in window
  );
}

/** Normalizes a rejected invoke() into our AppError shape. */
export function asAppError(e: unknown, fallbackCode = 'UNEXPECTED'): AppError {
  if (e && typeof e === 'object') {
    const o = e as Record<string, unknown>;
    if (typeof o.code === 'string' && typeof o.details === 'string') {
      return { code: o.code, details: o.details };
    }
  }
  return { code: fallbackCode, details: e instanceof Error ? e.message : String(e) };
}

// ---------------------------------------------------------------------------

class TauriBridge implements Bridge {
  readonly isDemo = false;
  protected async call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  detectTools() {
    return this.call<ToolStatus[]>('detect_tools');
  }
  setToolPath(tool: ToolName, path: string | null) {
    return this.call<ToolStatus[]>('set_tool_path', { tool, path });
  }
  checkToolPath(path: string) {
    return this.call<boolean>('check_tool_path', { path });
  }

  listDevices() {
    return this.call<Device[]>('list_devices');
  }
  getDeviceInfo(serial: string) {
    return this.call<DeviceInfo>('get_device_info', { serial });
  }
  getBattery(serial: string) {
    return this.call<BatteryInfo>('get_battery', { serial });
  }
  getStorage(serial: string) {
    return this.call<DiskUsage>('get_storage', { serial });
  }
  getNetworkInfo(serial: string) {
    return this.call<NetworkInfo>('get_network_info', { serial });
  }
  getDeviceProps(serial: string) {
    return this.call<Record<string, string>>('get_device_props', { serial });
  }
  adbConnect(host: string, port: number) {
    return this.call<string>('adb_connect', { host, port });
  }
  adbDisconnect(host: string, port: number) {
    return this.call<string>('adb_disconnect', { host, port });
  }
  adbServerVersion() {
    return this.call<string>('adb_server_version');
  }
  adbServerStart() {
    return this.call<string>('adb_server_start');
  }
  adbServerRestart(confirmation: string) {
    return this.call<string>('adb_server_restart', { confirmation });
  }
  rebootDevice(
    serial: string | null,
    target: 'system' | 'bootloader' | 'recovery' | 'sideload',
    confirmation: string,
  ) {
    return this.call<OpResult>('reboot_device', { serial, target, confirmation });
  }

  screenshot(serial: string | null, force: boolean) {
    return this.call<ScreenshotResult>('screenshot', { serial, force });
  }
  pickPath(kind: 'apk' | 'file' | 'directory') {
    return this.call<string | null>('pick_path', { kind });
  }
  openPath(path: string) {
    return this.call<void>('open_path', { path });
  }
  copyImageToClipboard(path: string) {
    return this.call<void>('copy_image_to_clipboard', { path });
  }

  scrcpyStart(serial: string, options: ScrcpyOptions) {
    return this.call<ScrcpyStatus>('scrcpy_start', { serial, options });
  }
  scrcpyStop() {
    return this.call<ScrcpyStatus>('scrcpy_stop');
  }
  scrcpyStatus() {
    return this.call<ScrcpyStatus>('scrcpy_status');
  }
  recordingDir() {
    return this.call<string>('recording_dir_cmd');
  }
  recordingFilename() {
    return this.call<string>('recording_filename');
  }

  shellOpen(serial: string) {
    return this.call<{ id: string; serial: string }>('shell_open', { serial });
  }
  shellWrite(id: string, data: string) {
    return this.call<void>('shell_write', { id, data });
  }
  shellClose(id: string) {
    return this.call<void>('shell_close', { id });
  }
  shellCloseAll() {
    return this.call<number>('shell_close_all');
  }

  listPackages(serial: string) {
    return this.call<PackageRow[]>('list_packages', { serial });
  }
  packageInfo(serial: string, pkg: string) {
    return this.call<PackageMeta>('package_info', { serial, pkg });
  }
  packageAction(
    serial: string | null,
    pkg: string,
    action:
      | 'open'
      | 'enable'
      | 'disable'
      | 'uninstall_for_user'
      | 'clear_data'
      | 'reinstall',
    user: number,
    confirmation?: string,
  ) {
    return this.call<OpResult>('package_action', { serial, pkg, action, user, confirmation });
  }
  packageExtract(serial: string, pkg: string, destDir?: string) {
    return this.call<string>('package_extract', { serial, pkg, destDir });
  }
  installApk(serial: string, local: string) {
    return this.call<OpResult>('execute_operation', {
      serial,
      op: { op: 'install_apk', local },
      confirmation: undefined,
    });
  }

  fileList(serial: string, path: string) {
    return this.call<FileEntry[]>('file_list', { serial, path });
  }
  fileMkdir(serial: string, path: string) {
    return this.call<OpResult>('file_mkdir', { serial, path });
  }
  fileRename(serial: string, from: string, to: string) {
    return this.call<OpResult>('file_rename', { serial, from, to });
  }
  fileDelete(serial: string, path: string, confirmation: string) {
    return this.call<OpResult>('file_delete', { serial, path, confirmation });
  }
  filePush(serial: string, local: string, remote?: string) {
    return this.call<string>('file_push', { serial, local, remote });
  }
  filePull(serial: string, remote: string, localDir?: string) {
    return this.call<string>('file_pull', { serial, remote, localDir });
  }
  transferCancel(id: string) {
    return this.call<void>('transfer_cancel', { id });
  }

  logcatStart(serial: string, spec?: string) {
    return this.call<string>('logcat_start', { serial, spec });
  }
  logcatStop(id: string) {
    return this.call<void>('logcat_stop', { id });
  }
  saveLogFile(path: string, content: string) {
    return this.call<void>('save_log_file', { path, content });
  }

  describeOperation(op: DeviceOperation, serial: string | null) {
    return this.call<string>('describe_operation', { op, serial });
  }
  executeOperation(serial: string | null, op: DeviceOperation, confirmation?: string) {
    return this.call<OpResult>('execute_operation', { serial, op, confirmation });
  }
  executeBatch(
    serial: string,
    op: DeviceOperation,
    packages: string[],
    confirmation?: string,
  ) {
    return this.call<BatchResult>('execute_batch', { serial, op, packages, confirmation });
  }
  classifyPackages(packages: string[]) {
    return this.call<PackageRisk[]>('classify_packages', { packages });
  }
  debloatProfiles() {
    return this.call<DebloatProfile[]>('debloat_profiles');
  }

  fastbootDevices() {
    return this.call<FastbootDevice[]>('fastboot_devices');
  }
  fastbootExecute(serial: string | null, op: FastbootOperation, confirmation?: string) {
    return this.call<OpResult>('fastboot_execute', { serial, op, confirmation });
  }

  getSettings() {
    return this.call<Settings>('get_settings');
  }
  saveSettings(settings: Settings) {
    return this.call<Settings>('save_settings', { settings });
  }
  getAudit(limit?: number) {
    return this.call<AuditEntry[]>('get_audit', { limit });
  }
  clearAudit() {
    return this.call<void>('clear_audit');
  }
  getDeviceHistory() {
    return this.call<HistoryEntry[]>('get_device_history');
  }
  clearDeviceHistory() {
    return this.call<void>('clear_device_history');
  }
  updateDeviceMetadata(serial: string, alias: string | null, tags: string[], favorite: boolean) {
    return this.call<HistoryEntry[]>('update_device_metadata', { serial, alias, tags, favorite });
  }
  getAppInfo() {
    return this.call<AppInfo>('get_app_info');
  }
  getAppPaths() {
    return this.call<AppPaths>('get_app_paths');
  }

  on(event: BackendEvent, cb: (payload: unknown) => void): Unsubscribe {
    let cancel: (() => void) | undefined;
    let disposed = false;
    void import('@tauri-apps/api/event')
      .then((m) => m.listen(event, (e) => { if (!disposed) cb(e.payload); }))
      .then((un) => { if (disposed) un(); else cancel = un; })
      .catch(() => {});
    return () => { disposed = true; cancel?.(); };
  }

}

// Browser development uses the same-origin local service, never mock device data.
class LocalBridge extends TauriBridge {
  private ids = new Set<string>();
  private listeners = new Map<BackendEvent, Set<(payload: unknown) => void>>();
  private timer: ReturnType<typeof setTimeout> | undefined;

  protected async call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    if (command === 'get_settings') {
      try { return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem('zittodb-settings') || '{}') } as T; }
      catch { return { ...DEFAULT_SETTINGS } as T; }
    }
    if (command === 'save_settings') { localStorage.setItem('zittodb-settings', JSON.stringify(args.settings)); return args.settings as T; }
    if (command === 'save_log_file') {
      const url = URL.createObjectURL(new Blob([String(args.content)], { type: 'text/plain;charset=utf-8' }));
      const link = document.createElement('a'); link.href = url; link.download = String(args.path).split(/[\\/]/).pop() || 'logcat.txt';
      link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); return undefined as T;
    }
    let response: Response;
    try {
      response = await fetch('/api/adb', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Zittodb-Client': 'local' }, body: JSON.stringify({ command, args }), signal: AbortSignal.timeout(30000) });
    } catch { throw { code: 'LOCAL_UNAVAILABLE', details: 'Serviço local indisponível. Confira o terminal e execute npm run dev.' }; }
    const result = await response.json();
    if (!response.ok) throw result;
    if (command === 'shell_open') this.ids.add(result.value.id);
    if (command === 'logcat_start') this.ids.add(result.value);
    if (command === 'shell_close' || command === 'logcat_stop') this.ids.delete(String(args.id));
    return result.value as T;
  }

  on(event: BackendEvent, cb: (payload: unknown) => void): Unsubscribe {
    const callbacks = this.listeners.get(event) ?? new Set();
    callbacks.add(cb); this.listeners.set(event, callbacks);
    const poll = async () => {
      try {
        if (this.ids.size) {
          const events = await this.call<{ event: BackendEvent; payload: unknown }[]>('poll_events', { ids: [...this.ids] });
          for (const e of events) this.listeners.get(e.event)?.forEach((fn) => fn(e.payload));
        }
      } catch { /* Requests from UI expose errors; transient event failures retry. */ }
      if (this.listeners.size) this.timer = setTimeout(poll, 250);
      else this.timer = undefined;
    };
    if (this.timer === undefined) this.timer = setTimeout(poll, 0);
    return () => {
      callbacks.delete(cb);
      if (!callbacks.size) this.listeners.delete(event);
    };
  }
}

// ---------------------------------------------------------------------------

let bridge: Bridge | null = null;

export function getBridge(): Bridge {
  if (!bridge) {
    bridge = isTauri() ? new TauriBridge() : import.meta.env.DEV && import.meta.env.MODE !== 'demo' && import.meta.env.MODE !== 'test' ? new LocalBridge() : new MockBridge();
  }
  return bridge;
}

// MockBridge is defined in mock.ts (kept separate for testability).
import { MockBridge } from './mock';
