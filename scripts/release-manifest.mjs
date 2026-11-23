// Gera o latest.json que o atualizador do app lê na última release do GitHub.
//
// Uso (depois de `npm run build` com a chave de assinatura no ambiente):
//   node scripts/release-manifest.mjs [arquivo-de-notas.md]
//
// Saída: target/release/bundle/nsis/latest.json. Suba junto com o instalador na release vX.Y.Z.
// O GitHub troca espaços por pontos no nome dos anexos, por isso a URL usa "Shift.Downloader_...".

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const REPO = 'Ericklucenaa/Shift-Downloader';
const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const conf = JSON.parse(readFileSync(join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
const version = conf.version;
const nsis = join(root, 'target/release/bundle/nsis');
const local = `${conf.productName}_${version}_x64-setup.exe`;
const asset = local.replaceAll(' ', '.');
const sigPath = join(nsis, `${local}.sig`);

if (!existsSync(sigPath)) {
  console.error(`Assinatura não encontrada: ${sigPath}\nGere o build com TAURI_SIGNING_PRIVATE_KEY definido (veja docs/RELEASING.md).`);
  process.exit(1);
}

const notesFile = process.argv[2];
const manifest = {
  version,
  notes: notesFile ? readFileSync(notesFile, 'utf8').trim() : `Shift Downloader ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: readFileSync(sigPath, 'utf8').trim(),
      url: `https://github.com/${REPO}/releases/download/v${version}/${asset}`
    }
  }
};

const out = join(nsis, 'latest.json');
writeFileSync(out, JSON.stringify(manifest, null, 2) + '\n');
console.log(`latest.json da versão ${version} gerado em ${out}`);
console.log(`Anexos da release v${version}: "${join(nsis, local)}#${asset}" e "${out}"`);
