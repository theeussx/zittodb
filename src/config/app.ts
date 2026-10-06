// Centralized app identity + defaults (spec §70):
// changing branding/theme/defaults happens here, not scattered in components.
//
// Brand: "Zittodb" = "zitto" (zero / silêncio) + "db" (Android Debug Bridge).
// The "db" is the Android Debug Bridge — not a database.
import packageJson from '../../package.json';

export const APP = {
  name: 'Zittodb',
  tagline: 'Android Debug Bridge, direto e local.',
  taglineEn: 'Android Debug Bridge, direct and local.',
  version: packageJson.version,
  identifier: 'app.zittodb.desktop',
} as const;

export const DEFAULT_SETTINGS = {
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
} as const;

/** scrcpy presets (spec §14). "low" = weak computers: 720p / 30fps / 2M. */
export const SCRCPY_PRESETS = {
  low: { label: 'Baixo consumo', maxSize: 1280, maxFps: 30, bitrate: '2M' },
  balanced: { label: 'Equilibrado', maxSize: 1920, maxFps: 60, bitrate: '4M' },
  high: { label: 'Alta qualidade', maxSize: null, maxFps: 120, bitrate: '8M' },
  custom: { label: 'Personalizado', maxSize: null, maxFps: null, bitrate: null },
} as const;

export type ScrcpyPresetId = keyof typeof SCRCPY_PRESETS;

export const BITRATE_OPTIONS = ['1M', '2M', '4M', '6M', '8M', '12M'] as const;
export const SIZE_OPTIONS = [720, 960, 1280, 1440, 1920, 2560] as const;
export const FPS_OPTIONS = [15, 24, 30, 60, 120] as const;
export const ORIENTATIONS = ['auto', 'portrait', 'landscape'] as const;

export const SHORTCUTS = {
  shell: 'Ctrl+Shift+A',
  screenshot: 'Ctrl+Shift+S',
  files: 'Ctrl+Shift+F',
  devices: 'Ctrl+Shift+D',
  logs: 'Ctrl+Shift+L',
} as const;

export const LOGCAT_LEVELS = ['E', 'W', 'I', 'D', 'V'] as const;
export const LOGCAT_TAGS = ['USB', 'Bluetooth', 'Camera', 'ActivityManager', 'Wifi', 'AudioFlinger'] as const;

export const DEVICE_ROOT = '/sdcard';
