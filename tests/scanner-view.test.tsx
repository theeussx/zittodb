import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScannerView } from '../src/features/scanner/ScannerView';
import { getBridge } from '../src/services/bridge';
import { useApp } from '../src/stores/app';

// The demo bridge is a singleton: start every test with an empty scan history.
beforeEach(async () => {
  const bridge = getBridge();
  for (const h of await bridge.scanHistory()) await bridge.scanDelete(h.scanId);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const t = () => useApp.getState().t;

async function runScan(name?: RegExp) {
  fireEvent.click(screen.getByRole('button', { name: name ?? new RegExp(`^${t()('scan.run')}$`) }));
  await screen.findByText(new RegExp(t()('scan.notProof').slice(0, 20)));
}

describe('ScannerView', () => {
  it('starts empty, scans the computer and shows counts, checks and limitations', async () => {
    render(<ScannerView />);
    expect(screen.getByText(t()('scan.empty'))).toBeTruthy();
    await runScan();
    expect(screen.getByText(t()('scan.summary.findings', { n: 1 }))).toBeTruthy();
    // checks that were NOT evaluated are surfaced, never hidden
    expect(screen.getByText(new RegExp(t()('scan.summary.notEvaluated', { n: 1 })))).toBeTruthy();
    fireEvent.click(screen.getByText(t()('scan.checks')));
    expect(screen.getAllByText(t()('scan.check.skipped')).length).toBeGreaterThan(0);
    expect(screen.getByText(t()('scan.limitations'))).toBeTruthy();
  });

  it('expands a finding to show detection, evidence, impact and recommendation', async () => {
    render(<ScannerView />);
    await runScan();
    fireEvent.click(screen.getByRole('button', { name: /scrcpy|Demo|demo/i }));
    expect(screen.getByText(t()('scan.field.evidence'))).toBeTruthy();
    expect(screen.getByText(t()('scan.field.recommendation'))).toBeTruthy();
    expect(screen.getByText('scrcpy.found')).toBeTruthy();
  });

  it('filters by severity and by free text', async () => {
    useApp.setState({ devices: [{ serial: 'DEMO1', state: 'connected', model: 'Demo', product: null, device: null, connection: 'usb', isEmulator: false }] });
    render(<ScannerView />);
    fireEvent.change(screen.getByLabelText(t()('scan.target')), { target: { value: 'device' } });
    await runScan();
    expect(screen.getByText(t()('scan.summary.findings', { n: 2 }))).toBeTruthy();

    fireEvent.change(screen.getByLabelText(t()('scan.filter.severity')), { target: { value: 'critical' } });
    expect(screen.getByText(t()('scan.noMatch'))).toBeTruthy();
    fireEvent.change(screen.getByLabelText(t()('scan.filter.severity')), { target: { value: 'all' } });

    fireEvent.change(screen.getByLabelText(t()('scan.filter.search')), { target: { value: '5555' } });
    expect(screen.getAllByRole('button', { expanded: false })).toHaveLength(1);
  });

  it('a device scan needs an explicit device', async () => {
    useApp.setState({ devices: [] });
    render(<ScannerView />);
    fireEvent.change(screen.getByLabelText(t()('scan.target')), { target: { value: 'device' } });
    expect(screen.getByText(t()('scan.noDevice'))).toBeTruthy();
    expect((screen.getByRole('button', { name: t()('scan.run') }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('compares with the previous scan of the same target', async () => {
    render(<ScannerView />);
    await runScan();
    expect((screen.getByRole('button', { name: t()('scan.compare') }) as HTMLButtonElement).disabled).toBe(true);
    await runScan();
    await waitFor(() => expect((screen.getByRole('button', { name: t()('scan.compare') }) as HTMLButtonElement).disabled).toBe(false));
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: t()('scan.compare') })); });
    const panel = await screen.findByLabelText(t()('scan.diff.title'));
    expect(within(panel).getByText(t()('scan.diff.counts', { n: 0, r: 0, c: 0, u: 1 }))).toBeTruthy();
    expect(within(panel).getByText(t()('scan.diff.note'))).toBeTruthy();
  });

  it('exports the stored report to a new file in the chosen folder', async () => {
    const bridge = getBridge();
    vi.spyOn(bridge, 'pickPath').mockResolvedValue('/tmp/out');
    const save = vi.spyOn(bridge, 'saveLogFile').mockResolvedValue();
    render(<ScannerView />);
    await runScan();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: t()('scan.export.json') })); });
    await waitFor(() => expect(save).toHaveBeenCalled());
    const [path, content] = save.mock.calls[0];
    expect(path).toMatch(/^\/tmp\/out\/zittodb-scan-\d+-[0-9a-f]{4}\.json$/);
    expect(JSON.parse(content).target).toBe('environment');
  });

  it('does nothing when the folder picker is cancelled', async () => {
    const bridge = getBridge();
    vi.spyOn(bridge, 'pickPath').mockResolvedValue(null);
    const save = vi.spyOn(bridge, 'saveLogFile').mockResolvedValue();
    render(<ScannerView />);
    await runScan();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: t()('scan.export.md') })); });
    expect(save).not.toHaveBeenCalled();
  });
});
