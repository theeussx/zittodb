import { APP } from '../config/app';

/** Repository used for release links and the browser-only fallback check. */
export const REPO_SLUG = 'theeussx/zittodb';
const RELEASES_API = `https://api.github.com/repos/${REPO_SLUG}/releases/latest`;
const TAGS_API = `https://api.github.com/repos/${REPO_SLUG}/tags?per_page=1`;
const REPOSITORY_URL = `https://github.com/${REPO_SLUG}`;

export type UpdateProgress =
  | { event: 'Started'; contentLength?: number }
  | { event: 'Progress'; chunkLength: number }
  | { event: 'Finished' };

export interface UpdateInfo {
  version: string;
  url: string;
  name: string | null;
  notes?: string | null;
  /** Present only in the desktop app, where the Tauri updater can install it. */
  install?: (onProgress?: (progress: UpdateProgress) => void) => Promise<void>;
}

function versionParts(version: string): [number, number, number] | null {
  const match = version.trim().replace(/^v/i, '').match(/^(\d+)\.(\d+)(?:\.(\d+))?/);
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), Number(match[3] ?? 0)];
}

export function isNewerVersion(current: string, latest: string): boolean {
  const currentParts = versionParts(current);
  const latestParts = versionParts(latest);
  if (!currentParts || !latestParts) return false;
  for (let i = 0; i < currentParts.length; i += 1) {
    if (latestParts[i] !== currentParts[i]) return latestParts[i] > currentParts[i];
  }
  return false;
}

function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Checks the signed Tauri endpoint on desktop and GitHub release metadata in
 * browser/demo mode. Installation is explicit: the app never replaces itself
 * silently while the user is working.
 */
export async function checkForUpdate(currentVersion = APP.version): Promise<UpdateInfo | null> {
  if (isDesktopRuntime()) {
    const [{ check }, { relaunch }] = await Promise.all([
      import('@tauri-apps/plugin-updater'),
      import('@tauri-apps/plugin-process'),
    ]);
    const update = await check();
    if (!update || !isNewerVersion(currentVersion, update.version)) return null;
    return {
      version: update.version.replace(/^v/i, ''),
      url: `${REPOSITORY_URL}/releases/latest`,
      name: null,
      notes: update.body ?? null,
      install: async (onProgress) => {
        await update.downloadAndInstall((event) => {
          if (event.event === 'Started') onProgress?.({ event: 'Started', contentLength: event.data.contentLength });
          if (event.event === 'Progress') onProgress?.({ event: 'Progress', chunkLength: event.data.chunkLength });
          if (event.event === 'Finished') onProgress?.({ event: 'Finished' });
        });
        await relaunch();
      },
    };
  }

  const requestInit: RequestInit = {
    headers: { Accept: 'application/vnd.github+json' },
    signal: AbortSignal.timeout(8000),
  };
  const response = await fetch(RELEASES_API, requestInit);
  let data: {
    tag_name?: unknown;
    name?: unknown;
    html_url?: unknown;
    body?: unknown;
    draft?: unknown;
    prerelease?: unknown;
  } = response.ok ? await response.json() : {};
  if (!response.ok && response.status === 404) {
    const tagsResponse = await fetch(TAGS_API, requestInit);
    if (tagsResponse.ok) {
      const tags = (await tagsResponse.json()) as Array<{ name?: unknown }>;
      const tag = tags[0]?.name;
      if (typeof tag === 'string') data = { tag_name: tag, html_url: `${REPOSITORY_URL}/tree/${tag}` };
    }
  }
  const version = typeof data.tag_name === 'string' ? data.tag_name.replace(/^v/i, '') : '';
  const url = typeof data.html_url === 'string' ? data.html_url : '';
  if (!version || !url || data.draft === true || data.prerelease === true || !isNewerVersion(currentVersion, version)) {
    return null;
  }
  const result: UpdateInfo = {
    version,
    url,
    name: typeof data.name === 'string' && data.name.trim() ? data.name : null,
  };
  if (typeof data.body === 'string') result.notes = data.body;
  return result;
}
