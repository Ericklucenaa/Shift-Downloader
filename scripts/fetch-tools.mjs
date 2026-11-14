// Baixa as ferramentas de mídia em versões fixas, confere o SHA-256 e extrai em
// apps/desktop/src-tauri/tools/. Os binários não vão para o git (são grandes).
// Uso: node scripts/fetch-tools.mjs
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync, readdirSync, cpSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tools = join(root, 'apps/desktop/src-tauri/tools');
const cache = join(root, 'target/tools-cache');

const TOOLS = [
  {
    name: 'yt-dlp 2026.10.26',
    url: 'https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp.exe',
    sha256: 'f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d',
    install: (file) => cpSync(file, join(tools, 'yt-dlp.exe'))
  },
  {
    name: 'FFmpeg 9.0.2 (BtbN, gpl-shared)',
    url: 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-13-12-43/ffmpeg-n9.0.2-57-gd838f9f9ae-win64-gpl-shared-9.0.zip',
    sha256: 'f4e068477787394da18a44b27d3e2cb19e05b21495b836a4fd5a31958b608932',
    install: (file) => {
      const tmp = join(cache, 'ffmpeg-x');
      rmSync(tmp, { recursive: true, force: true });
      mkdirSync(tmp, { recursive: true });
      // tar do Windows 10+ extrai zip; argumentos estruturados, sem shell.
      const tar = process.platform === 'win32' ? join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe') : 'tar';
      execFileSync(tar, ['-xf', file, '-C', tmp]);
      const top = readdirSync(tmp)[0];
      const dest = join(tools, 'ffmpeg');
      rmSync(dest, { recursive: true, force: true });
      mkdirSync(dest, { recursive: true });
      for (const f of readdirSync(join(tmp, top, 'bin'))) {
        if (f === 'ffplay.exe') continue; // não usamos o player
        cpSync(join(tmp, top, 'bin', f), join(dest, f));
      }
      cpSync(join(tmp, top, 'LICENSE.txt'), join(dest, 'LICENSE.txt'));
    }
  }
];

mkdirSync(tools, { recursive: true });
mkdirSync(cache, { recursive: true });

for (const t of TOOLS) {
  const file = join(cache, t.url.split('/').pop());
  if (!existsSync(file) || sha(file) !== t.sha256) {
    process.stdout.write(`Baixando ${t.name}… `);
    const res = await fetch(t.url);
    if (!res.ok) throw new Error(`${t.url}: HTTP ${res.status}`);
    writeFileSync(file + '.part', Buffer.from(await res.arrayBuffer()));
    renameSync(file + '.part', file);
    console.log('ok');
  }
  const got = sha(file);
  if (got !== t.sha256) {
    rmSync(file);
    throw new Error(`SHA-256 não confere para ${t.name}: ${got}`);
  }
  t.install(file);
  console.log(`✓ ${t.name} (sha256 conferido)`);
}

function sha(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}
