import { afterEach, describe, expect, it, vi } from 'vitest';
import { checkForUpdate, isNewerVersion } from '../src/services/updateService';

describe('update service', () => {
  afterEach(() => vi.restoreAllMocks());

  it('compares semantic versions without treating equal versions as updates', () => {
    expect(isNewerVersion('0.1.0', '0.2.0')).toBe(true);
    expect(isNewerVersion('0.1.0', 'v0.1.1')).toBe(true);
    expect(isNewerVersion('0.1.0', '0.1.0')).toBe(false);
    expect(isNewerVersion('0.2.0', '0.1.9')).toBe(false);
  });

  it('returns the latest stable release when it is newer', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        tag_name: 'v0.2.0',
        name: 'Android 15 compatibility',
        html_url: 'https://github.com/theeussx/zittodb/releases/tag/v0.2.0',
        draft: false,
        prerelease: false,
      }),
    }));
    await expect(checkForUpdate('0.1.0')).resolves.toEqual({
      version: '0.2.0',
      name: 'Android 15 compatibility',
      url: 'https://github.com/theeussx/zittodb/releases/tag/v0.2.0',
    });
  });

  it('ignores drafts and prereleases', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ tag_name: 'v0.2.0', html_url: 'https://example.test', prerelease: true }),
    }));
    await expect(checkForUpdate('0.1.0')).resolves.toBeNull();
  });

  it('falls back to the newest version tag when no release exists yet', async () => {
    vi.stubGlobal('fetch', vi.fn()
      .mockResolvedValueOnce({ ok: false, status: 404 })
      .mockResolvedValueOnce({ ok: true, json: async () => [{ name: 'v0.2.0' }] }));
    await expect(checkForUpdate('0.1.0')).resolves.toMatchObject({ version: '0.2.0' });
  });
});
