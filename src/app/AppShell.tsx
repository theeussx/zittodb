// App frame: sidebar (nav + tool status), topbar, routed content.

import { isTauri } from '../services/bridge';
import { APP, SHORTCUTS } from '../config/app';
import { useShortcuts } from '../hooks/useShortcuts';
import { useApp, type Nav } from '../stores/app';
import { Toasts } from '../components/Toasts';
import { UpdateBanner } from '../components/UpdateBanner';
import { DeviceList } from '../features/devices/DeviceList';
import { DeviceDashboard } from '../features/devices/DeviceDashboard';
import { CommandBuilder } from '../features/builder/CommandBuilder';
import { ScannerView } from '../features/scanner/ScannerView';
import { FastbootView } from '../features/fastboot/FastbootView';
import { HistoryView } from '../features/history/HistoryView';
import { SettingsView } from '../features/settings/SettingsView';
import type { UpdateInfo } from '../services/updateService';

function ToolStatusRow({ name, found, version }: { name: string; found: boolean; version: string | null }) {
  const t = useApp((s) => s.t);
  return (
    <div className={`tool-row ${found ? 'ok' : ''}`} title={version ?? t('tools.missing', { tool: name })}>
      <span className="dot" aria-hidden />
      <span className="tool-name">{name}</span>
      <span className="tool-detail">{found ? (version ?? t('tools.found', { tool: name })) : t('tools.missing', { tool: name })}</span>
    </div>
  );
}

function Sidebar() {
  const nav = useApp((s) => s.nav);
  const setNav = useApp((s) => s.setNav);
  const tools = useApp((s) => s.tools);
  const settings = useApp((s) => s.settings);
  const t = useApp((s) => s.t);

  const navItems: { id: Nav; label: string; icon: string; kbd?: string }[] = [
    { id: 'devices', label: t('nav.devices'), icon: '▣', kbd: SHORTCUTS.devices },
    { id: 'scanner', label: t('nav.scanner'), icon: '🔍' },
    { id: 'builder', label: t('nav.builder'), icon: '⌨' },
    { id: 'fastboot', label: t('nav.fastboot'), icon: '⚡' },
    { id: 'history', label: t('nav.history'), icon: '🕘' },
    { id: 'settings', label: t('nav.settings'), icon: '⚙' },
  ];

  return (
    <aside className="sidebar" aria-label="Navegação">
      <div className="brand">
        <div className="brand-logo" aria-hidden>
          {'>_'}
        </div>
        <div>
          <div className="brand-name">{APP.name}</div>
          <div className="brand-sub">v{APP.version} · Linux</div>
        </div>
      </div>

      <nav className="nav">
        {navItems.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${nav === item.id ? 'active' : ''}`}
            onClick={() => setNav(item.id)}
          >
            <span aria-hidden>{item.icon}</span>
            {item.label}
            {item.kbd && <span className="kbd">{item.kbd}</span>}
          </button>
        ))}
      </nav>

      <div className="tools-status" aria-label="Status das ferramentas">
        {tools.length > 0 && (
          <>
            <ToolStatusRow name="ADB" found={!!tools[0]?.found} version={tools[0]?.version ?? null} />
            <ToolStatusRow name="scrcpy" found={!!tools[1]?.found} version={tools[1]?.version ?? null} />
            <ToolStatusRow name="fastboot" found={!!tools[2]?.found} version={tools[2]?.version ?? null} />
          </>
        )}
        <div className={`perf-badge ${settings.performanceMode === 'normal' ? '' : settings.performanceMode}`}>
          <span aria-hidden>●</span>
          {settings.performanceMode === 'normal'
            ? t('settings.perf.normal')
            : settings.performanceMode === 'low'
              ? t('settings.perf.low')
              : t('settings.perf.ultra')}
        </div>
      </div>
    </aside>
  );
}

export function AppShell({ update, onDismissUpdate }: { update?: UpdateInfo | null; onDismissUpdate?: () => void }) {
  const nav = useApp((s) => s.nav);
  const devices = useApp((s) => s.devices);
  const selectedSerial = useApp((s) => s.selectedSerial);
  const isDemo = useApp((s) => s.isDemo);
  const t = useApp((s) => s.t);

  useShortcuts();

  const selected = devices.find((d) => d.serial === selectedSerial) ?? null;

  return (
    <div className="app">
      <Sidebar />
      <main className="main">
        <div className="topbar">
          <h1>
            {nav === 'devices'
              ? selected
                ? selected.model ?? selected.serial
                : t('nav.devices')
              : t(`nav.${nav}`)}
          </h1>
          {selected && (
            <span className="selected-chip">
              <span className={`state-dot ${selected.state}`} aria-hidden />
              <span className="mono">{selected.serial}</span>
              <span>{t(`devices.state.${selected.state}`)}</span>
            </span>
          )}
          <div className="spacer" />
          <span className="dim small">{APP.tagline}</span>
        </div>

        {isDemo && (
          <div className="banner" role="note">
            <span aria-hidden>◍</span>
            <span>{t('demo.banner')}</span>
          </div>
        )}

        {!isDemo && !isTauri() && (
          <div className="banner" role="note">{t('local.banner')}</div>
        )}
        {update && onDismissUpdate && <UpdateBanner update={update} onDismiss={onDismissUpdate} />}
        <div className="content">
          {nav === 'devices' &&
            (selected ? <DeviceDashboard /> : <DeviceList />)}
          {nav === 'scanner' && <ScannerView />}
          {nav === 'builder' && <CommandBuilder />}
          {nav === 'fastboot' && <FastbootView />}
          {nav === 'history' && <HistoryView />}
          {nav === 'settings' && <SettingsView />}
        </div>
      </main>
      <Toasts />
    </div>
  );
}
