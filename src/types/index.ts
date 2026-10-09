// DTO types mirroring the Rust backend (serde camelCase).

export type ToolName = 'adb' | 'scrcpy' | 'fastboot';

export interface ToolStatus {
  name: ToolName;
  found: boolean;
  path: string | null;
  version: string | null;
  source: string | null;
}

export type DeviceState = 'connected' | 'offline' | 'unauthorized' | 'recovery' | 'other';
export type ConnectionKind = 'usb' | 'wifi' | 'emulator' | 'unknown';

export interface Device {
  serial: string;
  state: DeviceState;
  model: string | null;
  product: string | null;
  device: string | null;
  connection: ConnectionKind;
  isEmulator: boolean;
}

export interface DeviceInfo {
  serial: string;
  model: string | null;
  manufacturer: string | null;
  brand: string | null;
  androidVersion: string | null;
  sdkVersion: string | null;
  securityPatch: string | null;
  buildId: string | null;
  buildDisplay: string | null;
  kernel: string | null;
  cpuAbis: string | null;
  boardPlatform: string | null;
  hardware: string | null;
  product: string | null;
  device: string | null;
  totalRamMb: number | null;
}

export interface BatteryInfo {
  level: number | null;
  temperature: number | null; // 0.1 °C
  temperatureC: number | null;
  status: string | null;
  health: string | null;
  technology: string | null;
  voltageMv: number | null;
  plugged: string | null;
}

export interface DiskUsage {
  totalMb: number;
  usedMb: number;
  availMb: number;
  path: string;
}

export interface NetIface {
  name: string;
  ip: string;
  prefix: number;
}

export interface NetworkInfo {
  interfaces: NetIface[];
  gateway: string | null;
  dns: string[];
  mac: string | null;
}

export interface PackageRow {
  name: string;
  version: string | null;
  versionCode: number | null;
  uid: string | null;
  path: string | null;
  isSystem: boolean;
  isDisabled: boolean;
}

export interface PackageMeta {
  name: string;
  versionName: string | null;
  versionCode: number | null;
  uid: string | null;
  codePath: string | null;
  firstInstall: number | null;
  lastUpdate: number | null;
}

export interface FileEntry {
  name: string;
  isDir: boolean;
  isLink: boolean;
  size: number | null;
  modified: string | null;
  permissions: string;
}

export interface OpResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  code: string | null;
}

export interface UndoRef {
  action: string;
  package: string;
}

export interface AuditEntry {
  ts: number;
  device: string | null;
  action: string;
  command: string;
  result: string;
  undo: UndoRef | null;
}

export interface HistoryEntry {
  serial: string;
  model: string | null;
  lastState: string | null;
  lastSeenMs: number | null;
  connection: string | null;
  alias?: string | null;
  tags: string[];
  favorite: boolean;
}

export type Theme = 'dark' | 'light' | 'system';
export type Language = 'auto' | 'pt-BR' | 'en-US';
export type PerfMode = 'normal' | 'low' | 'ultra';

export interface Settings {
  theme: Theme;
  language: Language;
  performanceMode: PerfMode;
  adbPath: string | null;
  scrcpyPath: string | null;
  fastbootPath: string | null;
  screenshotDir: string | null;
  recordingDir: string | null;
  downloadDir: string | null;
  autoRefreshSecs: number | null;
  logcatMaxLines: number;
  auditEnabled: boolean;
}

export interface ScrcpyOptions {
  preset: string | null;
  maxSize: number | null;
  maxFps: number | null;
  bitrate: string | null;
  orientation: string | null;
  turnScreenOff: boolean;
  audio: boolean;
  alwaysOnTop: boolean;
  recordPath: string | null;
}

export interface ScrcpyStatus {
  running: boolean;
  pid: number | null;
  recording: boolean;
}

export interface TransferEvent {
  id: string;
  status: 'running' | 'done' | 'error' | 'cancelled';
  doneBytes: number;
  total: number | null;
  message: string;
}

export type DeviceOperation =
  | { op: 'get_props' }
  | { op: 'get_kernel' }
  | { op: 'get_meminfo' }
  | { op: 'get_disk_usage'; path: string }
  | { op: 'get_battery' }
  | { op: 'get_network' }
  | { op: 'get_resolv' }
  | { op: 'get_mac'; iface: string }
  | { op: 'screenshot' }
  | { op: 'list_packages'; thirdParty: boolean; disabled: boolean }
  | { op: 'package_path'; pkg: string }
  | { op: 'package_dump'; pkg: string }
  | { op: 'open_package'; pkg: string }
  | { op: 'enable_package'; pkg: string }
  | { op: 'disable_package'; pkg: string; user: number }
  | { op: 'uninstall_for_user'; pkg: string; user: number }
  | { op: 'reinstall_existing'; pkg: string }
  | { op: 'clear_package_data'; pkg: string }
  | { op: 'install_apk'; local: string }
  | { op: 'list_dir'; path: string }
  | { op: 'mkdir'; path: string }
  | { op: 'rename'; from: string; to: string }
  | { op: 'delete'; path: string }
  | { op: 'push'; local: string; remote: string }
  | { op: 'pull'; remote: string; local: string }
  | { op: 'reboot'; target: 'system' | 'bootloader' | 'recovery' | 'sideload' }
  | { op: 'connect'; host: string; port: number }
  | { op: 'disconnect'; host: string; port: number };

export interface FastbootDevice {
  serial: string;
  state: string;
}

export type FastbootOperation =
  | { op: 'devices' }
  | { op: 'reboot'; target: 'system' | 'bootloader' | 'recovery' | 'sideload' }
  | { op: 'getvar'; var: string }
  | { op: 'flash'; partition: string; file: string }
  | { op: 'erase'; partition: string }
  | { op: 'unlock' }
  | { op: 'lock' };

export type RiskLevel = 'SAFE' | 'LOW_RISK' | 'CAUTION' | 'DANGEROUS' | 'CRITICAL' | 'UNKNOWN';

export interface PackageRisk {
  package: string;
  risk: RiskLevel;
}

export interface DebloatProfile {
  id: string;
  name: string;
  maxRisk: RiskLevel;
  description: string;
}

export interface BatchItem {
  index: number;
  package: string;
  ok: boolean;
  code: string | null;
  message: string;
}

export interface BatchResult {
  serial: string;
  results: BatchItem[];
}

export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
  platform: string;
}

export interface AppPaths {
  configDir: string;
  dataDir: string;
  logFile: string;
  auditFile: string;
}

export interface ScreenshotResult {
  path: string;
  sizeBytes: number;
}

/** Normalized backend error (AppError from Rust). */
export interface AppError {
  code: string;
  details: string;
}

export const DIAGNOSTIC_REPORT_SCHEMA_VERSION = 1 as const;

export type ReportSectionStatus = 'ok' | 'unavailable' | 'error';

export interface ReportError {
  code: string;
  details: string;
}

export interface ReportSection {
  status: ReportSectionStatus;
  data: unknown | null;
  error: ReportError | null;
}

export interface DiagnosticReport {
  schemaVersion: typeof DIAGNOSTIC_REPORT_SCHEMA_VERSION;
  generatedAt: string;
  appVersion: string;
  device: {
    serial: string | null;
    state: string | null;
  };
  tools: Array<{
    name: string;
    found: boolean;
    version: string | null;
  }>;
  sections: Record<string, ReportSection>;
  limitations: string[];
}

export interface ReportPrivacy {
  includeSerial: boolean;
  includeNetwork: boolean;
}

export interface DiagnosticReportExport {
  report: DiagnosticReport;
  jsonPath: string;
  markdownPath: string;
}

export type BackendEvent = 'shell-output' | 'logcat-line' | 'scrcpy-log' | 'file-progress';
