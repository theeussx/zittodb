import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = process.cwd();
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');
const packageVersion = JSON.parse(read('package.json')).version;
const cargoVersion = read('src-tauri/Cargo.toml').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const tauriVersion = JSON.parse(read('src-tauri/tauri.conf.json')).version;
const readmeVersion = read('README.md').match(/release-v([^-/]+)-3ddc84/)?.[1];

const versions = {
  'package.json': packageVersion,
  'src-tauri/Cargo.toml': cargoVersion,
  'src-tauri/tauri.conf.json': tauriVersion,
  'README.md badge': readmeVersion,
};
const missing = Object.entries(versions).filter(([, version]) => !version);
const distinct = new Set(Object.values(versions));

if (missing.length || distinct.size !== 1) {
  console.error('Versiones do projeto estão fora de sincronia:');
  for (const [source, version] of Object.entries(versions)) console.error(`  ${source}: ${version ?? '<ausente>'}`);
  process.exit(1);
}

console.log(`Versões sincronizadas: v${packageVersion}`);
