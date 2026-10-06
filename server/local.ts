import { execFile, spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { promisify } from 'node:util';
import type { Plugin } from 'vite';
const exec = promisify(execFile);
const fail = (details: string, code = 'INVALID_ARGUMENT'): never => { throw { code, details }; };
export function parseDevices(text: string) {
  return text.split('\n').slice(1).filter((l) => l.trim() && !l.startsWith('*')).map((line) => {
    const [serial, status, ...attrs] = line.trim().split(/\s+/);
    const value = (key: string) => attrs.find((a) => a.startsWith(`${key}:`))?.slice(key.length + 1) ?? null;
    return { serial, state: status === 'device' ? 'connected' : ['offline', 'unauthorized', 'recovery'].includes(status) ? status : 'other', model: value('model'), product: value('product'), device: value('device'), isEmulator: serial.startsWith('emulator-'), connection: serial.startsWith('emulator-') ? 'emulator' : serial.includes(':') ? 'wifi' : 'usb' };
  });
}
export function validSerial(value: unknown): string {
  if (typeof value !== 'string' || !/^[a-zA-Z0-9_.:\[\]-]{1,200}$/.test(value) || value.startsWith('-')) return fail('Serial inválido.');
  return value;
}
export function localBackend(): Plugin {
  const sessions = new Map<string, { child: ChildProcessWithoutNullStreams; event: string; queue: string[]; touched: number }>();
  const stop = (id: string) => { sessions.get(id)?.child.kill(); sessions.delete(id); };
  const run = async (args: string[]) => {
    try { return (await exec('adb', args, { timeout: 20000, maxBuffer: 8 * 1024 * 1024 })).stdout; }
    catch (e) { const error = e as NodeJS.ErrnoException & { stderr?: string }; return fail(error.code === 'ENOENT' ? 'ADB não encontrado. Instale android-tools-adb e confira adb devices no terminal.' : error.stderr || error.message, error.code === 'ENOENT' ? 'TOOL_NOT_FOUND' : 'ADB_FAILED'); }
  };
  const open = async (serial: string, args: string[], event: string) => {
    if (sessions.size >= 8) fail('Limite de sessões atingido. Feche uma sessão.');
    const child = spawn('adb', ['-s', validSerial(serial), ...args], { stdio: 'pipe' });
    await new Promise<void>((resolve, reject) => { child.once('spawn', resolve); child.once('error', reject); });
    const id = randomUUID();
    const session = { child, event, queue: [] as string[], touched: Date.now() };
    sessions.set(id, session);
    const append = (data: Buffer) => { session.queue.push(data.toString()); session.queue = session.queue.slice(-500); };
    child.stdin.on('error', (e) => append(Buffer.from(e.message)));
    child.stdout.on('data', append); child.stderr.on('data', append);
    child.on('error', (e) => append(Buffer.from(e.message)));
    child.on('exit', () => append(Buffer.from('\n[processo encerrado]\n')));
    return id;
  };
  const dispatch = async (command: string, a: Record<string, any>): Promise<unknown> => {
    const adb = (...args: string[]) => run(['-s', validSerial(a.serial), ...args]);
    switch (command) {
      case 'detect_tools': return Promise.all(['adb', 'scrcpy', 'fastboot'].map(async (name) => { try { const { stdout } = await exec(name, [name === 'adb' ? 'version' : '--version'], { timeout: 5000 }); return { name, found: true, path: name, version: stdout.split('\n')[0], source: 'PATH' }; } catch { return { name, found: false, path: null, version: null, source: null }; } }));
      case 'adb_server_version': return run(['version']);
      case 'adb_server_start': return run(['start-server']);
      case 'adb_server_restart': {
        if (a.confirmation !== 'REINICIAR_ADB') fail('Digite REINICIAR_ADB para reiniciar o servidor ADB.', 'CONFIRMATION_REQUIRED');
        await run(['kill-server']);
        return run(['start-server']);
      }
      case 'list_devices': return parseDevices(await run(['devices', '-l']));
      case 'get_device_props': {
        const text = await adb('shell', 'getprop');
        return Object.fromEntries([...text.matchAll(/^\[([^\]]+)\]: \[(.*)\]\r?$/gm)].map((m) => [m[1], m[2]]));
      }
      case 'get_device_info': {
        const p = await dispatch('get_device_props', a) as Record<string, string>;
        return { serial: a.serial, model: p['ro.product.model'], manufacturer: p['ro.product.manufacturer'], brand: p['ro.product.brand'], androidVersion: p['ro.build.version.release'], sdkVersion: p['ro.build.version.sdk'], securityPatch: p['ro.build.version.security_patch'], buildDisplay: p['ro.build.display.id'], cpuAbis: p['ro.product.cpu.abilist'], boardPlatform: p['ro.board.platform'], kernel: (await adb('shell', 'uname', '-r')).trim(), totalRamMb: null };
      }
      case 'get_battery': {
        const text = await adb('shell', 'dumpsys', 'battery');
        const fields = Object.fromEntries(text.split('\n').map((l) => l.trim().split(/:\s*/, 2)));
        const num = (k: string) => fields[k] == null ? null : Number(fields[k]);
        return { level: num('level'), temperature: num('temperature'), temperatureC: fields.temperature ? Number(fields.temperature) / 10 : null, status: fields.status, health: fields.health, technology: fields.technology, voltageMv: num('voltage'), plugged: fields['USB powered'] === 'true' ? '2' : fields['AC powered'] === 'true' ? '1' : '0' };
      }
      case 'get_storage': {
        const row = (await adb('shell', 'df', '-k', '/data')).trim().split('\n').pop()!.trim().split(/\s+/);
        if (row.length < 6 || !Number.isFinite(Number(row[1]))) fail('Não foi possível interpretar df do dispositivo.', 'ADB_FAILED');
        return { totalMb: Number(row[1]) / 1024, usedMb: Number(row[2]) / 1024, availMb: Number(row[3]) / 1024, path: '/data' };
      }
      case 'list_packages': {
        const [all, system, disabled] = await Promise.all([adb('shell', 'pm', 'list', 'packages'), adb('shell', 'pm', 'list', 'packages', '-s'), adb('shell', 'pm', 'list', 'packages', '-d')]);
        const names = (s: string) => s.split('\n').filter((l) => l.startsWith('package:')).map((l) => l.slice(8).trim());
        const sys = new Set(names(system)), dis = new Set(names(disabled));
        return names(all).map((name) => ({ name, version: null, versionCode: null, uid: null, path: null, isSystem: sys.has(name), isDisabled: dis.has(name) }));
      }
      case 'shell_open': return { id: await open(a.serial, ['shell'], 'shell-output'), serial: a.serial };
      case 'shell_write': {
        const s = sessions.get(a.id);
        if (!s || s.event !== 'shell-output' || typeof a.data !== 'string' || a.data.length > 10000) fail('Sessão shell inválida.');
        s!.touched = Date.now(); s!.child.stdin.write(`${a.data}\n`); return null;
      }
      case 'logcat_start': {
        if (a.spec && (typeof a.spec !== 'string' || !/^[\w.*-]+(?::[VDIWEFS])?$/.test(a.spec))) fail('Filtro inválido. Use TAG:PRIORIDADE.');
        return open(a.serial, ['logcat', '-v', 'threadtime', ...(a.spec ? [a.spec] : [])], 'logcat-line');
      }
      case 'shell_close': case 'logcat_stop': stop(a.id); return null;
      case 'shell_close_all': { const ids = [...sessions].filter(([, s]) => s.event === 'shell-output').map(([id]) => id); ids.forEach(stop); return ids.length; }
      case 'poll_events': return (Array.isArray(a.ids) ? a.ids : []).flatMap((id: string) => { const s = sessions.get(id); if (!s) return []; s.touched = Date.now(); return s.queue.splice(0).map((line) => ({ event: s.event, payload: { session: id, line } })); });
      default: return fail('Esta operação ainda requer o aplicativo desktop. Execute npm run tauri:dev. O modo local oferece dispositivos, informações, lista de aplicativos, shell e logcat.', 'UNSUPPORTED_LOCAL');
    }
  };
  return {
    name: 'local-adb',
    configureServer(server) {
      const timer = setInterval(() => { for (const [id, s] of sessions) if (Date.now() - s.touched > 60000) stop(id); }, 15000);
      timer.unref();
      server.httpServer?.once('close', () => { clearInterval(timer); [...sessions.keys()].forEach(stop); });
      server.middlewares.use('/api/adb', async (req, res) => {
        res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store');
        // Local machine only: reject DNS rebinding, foreign origins and simple CSRF requests.
        const host = req.headers.host ?? '';
        if (!/^(localhost|127\.0\.0\.1|\[::1\])(:\d+)?$/.test(host) || req.headers.origin !== `http://${host}` || req.method !== 'POST' || req.headers['content-type'] !== 'application/json' || req.headers['x-zittodb-client'] !== 'local') {
          res.statusCode = 403; res.end(JSON.stringify({ code: 'FORBIDDEN', details: 'API ADB disponível apenas na origem local.' })); return;
        }
        try {
          let body = '';
          for await (const chunk of req) { body += chunk; if (body.length > 65536) fail('Requisição muito grande.'); }
          const { command, args = {} } = JSON.parse(body);
          if (!args || typeof args !== 'object' || Array.isArray(args)) fail('Argumentos inválidos.');
          res.end(JSON.stringify({ value: await dispatch(command, args) }));
        } catch (e) { const error = e as { code?: string; details?: string; message?: string }; res.statusCode = 400; res.end(JSON.stringify({ code: error.code ?? 'UNEXPECTED', details: error.details ?? error.message ?? 'Falha no serviço local.' })); }
      });
    },
  };
}
