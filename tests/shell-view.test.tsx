import { cleanup, fireEvent, render, screen, waitFor, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ShellView } from '../src/features/shell/ShellView';
import { getBridge } from '../src/services/bridge';
import { useApp } from '../src/stores/app';
import { SHELL_CONFIRMATION_WORD } from '../src/types';

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

function setup(classify: (line: string) => 'read' | 'modify' | 'destructive') {
  const bridge = getBridge();
  vi.spyOn(bridge, 'shellOpen').mockResolvedValue({ id: 'sh1', serial: 'dev' });
  vi.spyOn(bridge, 'shellClose').mockResolvedValue();
  vi.spyOn(bridge, 'on').mockReturnValue(() => {});
  vi.spyOn(bridge, 'shellClassify').mockImplementation(async (line) => ({ risk: classify(line), reasons: [] }));
  const write = vi.spyOn(bridge, 'shellWrite').mockResolvedValue();
  const t = useApp.getState().t;
  render(<ShellView serial="dev" />);
  return { write, t };
}

async function typeAndEnter(t: ReturnType<typeof setup>['t'], text: string) {
  const input = await screen.findByPlaceholderText(t('shell.placeholder'));
  await waitFor(() => expect(input).toBeEnabled());
  fireEvent.change(input, { target: { value: text } });
  await act(async () => { fireEvent.keyDown(input, { key: 'Enter' }); });
  return input;
}

describe('ShellView risk gate', () => {
  it('runs read-only commands immediately, without confirmation', async () => {
    const { write, t } = setup(() => 'read');
    await typeAndEnter(t, 'getprop ro.product.model');
    expect(write).toHaveBeenCalledWith('sh1', 'getprop ro.product.model', undefined);
  });

  it('asks for the typed word before a destructive command and sends it only after', async () => {
    const { write, t } = setup(() => 'destructive');
    await typeAndEnter(t, 'rm -rf /sdcard/x');
    expect(write).not.toHaveBeenCalled();

    const field = await screen.findByPlaceholderText(t('common.confirmDestructive.placeholder', { word: SHELL_CONFIRMATION_WORD }));
    fireEvent.change(field, { target: { value: 'executar' } }); // wrong case
    await act(async () => { fireEvent.keyDown(field, { key: 'Enter' }); });
    expect(write).not.toHaveBeenCalled();

    fireEvent.change(field, { target: { value: SHELL_CONFIRMATION_WORD } });
    await act(async () => { fireEvent.keyDown(field, { key: 'Enter' }); });
    await waitFor(() => expect(write).toHaveBeenCalledWith('sh1', 'rm -rf /sdcard/x', SHELL_CONFIRMATION_WORD));
  });

  it('cancelling the dialog never sends the command', async () => {
    const { write, t } = setup(() => 'destructive');
    await typeAndEnter(t, 'reboot');
    fireEvent.click(await screen.findByRole('button', { name: t('common.cancel') }));
    expect(write).not.toHaveBeenCalled();
  });
});
