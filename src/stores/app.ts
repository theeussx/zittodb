// Global UI state (zustand — small, no boilerplate, spec §5: lightweight).

import { create } from 'zustand';
import { DEFAULT_SETTINGS } from '../config/app';
import { getBridge } from '../services/bridge';
import { resolveLanguage, translate } from '../i18n';
import type {
  Device,
  DeviceState,
  PerfMode,
  Settings,
  Theme,
  ToolStatus,
  Language,
} from '../types';

export type Nav = 'devices' | 'scanner' | 'builder' | 'fastboot' | 'history' | 'settings';
export type DeviceTab =
  | 'overview'
  | 'screen'
  | 'apps'
  | 'files'
  | 'shell'
  | 'logs'
  | 'diagnostics'
  | 'debloat';

export interface Toast {
  id: number;
  kind: 'info' | 'success' | 'error' | 'warning';
  title: string;
  body?: string;
}

interface AppState {
  // environment
  isDemo: boolean;
  settings: Settings;
  tools: ToolStatus[];
  toolsChecked: boolean;

  // devices
  devices: Device[];
  devicesLoading: boolean;
  selectedSerial: string | null;

  // navigation
  nav: Nav;
  deviceTab: DeviceTab;

  // ui
  toasts: Toast[];

  // actions
  setDemo: (v: boolean) => void;
  setSettings: (patch: Partial<Settings>) => void;
  applySettings: (settings: Settings) => void;
  updateSettings: (patch: Partial<Settings>) => void;
  refreshTools: () => Promise<void>;
  setTools: (tools: ToolStatus[]) => void;
  setDevices: (devices: Device[]) => void;
  setDevicesLoading: (v: boolean) => void;
  selectDevice: (serial: string | null) => void;
  setNav: (nav: Nav) => void;
  setDeviceTab: (tab: DeviceTab) => void;
  toast: (t: Omit<Toast, 'id'>) => void;
  dismissToast: (id: number) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

let toastId = 0;

const initialSettings: Settings = {
  theme: DEFAULT_SETTINGS.theme as Theme,
  language: DEFAULT_SETTINGS.language as Language,
  performanceMode: DEFAULT_SETTINGS.performanceMode as PerfMode,
  adbPath: null,
  scrcpyPath: null,
  fastbootPath: null,
  screenshotDir: null,
  recordingDir: null,
  downloadDir: null,
  autoRefreshSecs: null,
  logcatMaxLines: DEFAULT_SETTINGS.logcatMaxLines,
  auditEnabled: DEFAULT_SETTINGS.auditEnabled,
};

export const useApp = create<AppState>((set, get) => ({
  isDemo: false,
  settings: initialSettings,
  tools: [],
  toolsChecked: false,

  devices: [],
  devicesLoading: false,
  selectedSerial: null,

  nav: 'devices',
  deviceTab: 'overview',

  toasts: [],

  setDemo: (v) => set({ isDemo: v }),
  setSettings: (patch) => set({ settings: { ...get().settings, ...patch } }),
  applySettings: (settings) => {
    set({ settings });
    applyHtmlClasses(settings);
  },
  updateSettings: (patch) => {
    const settings = { ...get().settings, ...patch };
    set({ settings });
    applyHtmlClasses(settings);
  },
  refreshTools: async () => {
    try {
      const tools = await getBridge().detectTools();
      set({ tools, toolsChecked: true });
    } catch {
      // keep previous state; status shown in settings
    }
  },
  setTools: (tools) => set({ tools, toolsChecked: true }),
  setDevices: (devices) => set({ devices }),
  setDevicesLoading: (v) => set({ devicesLoading: v }),
  selectDevice: (serial) =>
    set({
      selectedSerial: serial,
      nav: serial ? 'devices' : get().nav,
      deviceTab: 'overview',
    }),
  setNav: (nav) => set({ nav }),
  setDeviceTab: (deviceTab) => set({ deviceTab }),

  toast: (t) => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { ...t, id }] });
    const ttl = t.kind === 'error' ? 8000 : 4500;
    setTimeout(() => get().dismissToast(id), ttl);
  },
  dismissToast: (id) => set({ toasts: get().toasts.filter((x) => x.id !== id) }),

  t: (key, params) =>
    translate(resolveLanguage(get().settings.language), key, params),
}));

/** Convenience: apply theme + performance mode to <html>. */
export function applyHtmlClasses(settings: Settings): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const systemDark =
    typeof window !== 'undefined' &&
    !!window.matchMedia?.('(prefers-color-scheme: dark)');
  const dark =
    settings.theme === 'dark' || (settings.theme === 'system' && (systemDark ?? true));
  root.dataset.theme = dark ? 'dark' : 'light';
  root.dataset.perf = settings.performanceMode;
}

/** Performance mode tweaks that are not CSS (spec §15, §40). */
export function perfTweaks(mode: PerfMode): {
  refreshIntervalMs: number | null; // null = no auto refresh
  logcatBuffer: number;
} {
  switch (mode) {
    case 'low':
      return { refreshIntervalMs: 15000, logcatBuffer: 3000 };
    case 'ultra':
      return { refreshIntervalMs: null, logcatBuffer: 1500 };
    default:
      return { refreshIntervalMs: 10000, logcatBuffer: 5000 };
  }
}

export function stateLabel(state: DeviceState): string {
  const t = useApp.getState().t;
  switch (state) {
    case 'connected':
      return t('devices.state.connected');
    case 'offline':
      return t('devices.state.offline');
    case 'unauthorized':
      return t('devices.state.unauthorized');
    case 'recovery':
      return t('devices.state.recovery');
    default:
      return t('devices.state.other');
  }
}
