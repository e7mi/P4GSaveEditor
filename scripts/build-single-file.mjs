// Build dist/P4GSaveEditor.html: the whole app in one file that runs on
// file://. ES module imports and wasm fetch() are blocked there, so the
// wasm-bindgen glue and app.js are inlined as classic scripts and the wasm
// binary is embedded as base64. Run after `wasm-bindgen --out-dir web/pkg`:
//   node scripts/build-single-file.mjs

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const web = join(root, 'web');

let html = readFileSync(join(web, 'index.html'), 'utf8');
const css = readFileSync(join(web, 'style.css'), 'utf8');
const favicon = readFileSync(join(web, 'favicon.ico'));
const wasm = readFileSync(join(web, 'pkg', 'p4g_wasm_bg.wasm'));

let glue = readFileSync(join(web, 'pkg', 'p4g_wasm.js'), 'utf8');
glue = glue
  .replace(/^import\s+[\s\S]*?from\s+['"][^'"]+['"]\s*;/gm, '')
  .replace(/^export\s*\{[^}]*\}\s*;?/m, '')
  .replace(/^export\s+(?=(const|let|var|function|class|async|default))/gm, '')
  // file:// で import.meta は構文エラー。デフォルト経路は使わず initSync を呼ぶので潰す
  .replace(/new URL\('p4g_wasm_bg\.wasm', import\.meta\.url\)/,
    "(() => { throw new Error('pass module_or_path on file://'); })()");

let app = readFileSync(join(web, 'app.js'), 'utf8');
app = app
  .replace(/^import\s+[\s\S]*?from\s+['"][^'"]+['"]\s*;/gm, '')
  .replace(
    'await init("./pkg/p4g_wasm_bg.wasm");',
    'initSync({ module: __P4G_WASM_BYTES });'
  );

// pure.js is a classic script shared with node tests; inline it before app.js
// when it exists (audit-fix branch layout). Missing is not an error.
let pure = '';
try {
  pure = readFileSync(join(web, 'pure.js'), 'utf8');
} catch { /* older layout without pure.js */ }

if (app.includes('await init(') || app.includes('import init'))
  throw new Error('wasm init call in app.js was not rewritten');
if (glue.includes('export '))
  throw new Error('unstripped export left in glue');

const esc = (s) => s.replace(/<\/script/gi, '<\\/script');
const scripts =
  `<script>var __P4G_WASM_BYTES = Uint8Array.fromBase64("${wasm.toString('base64')}");</script>\n` +
  `<script>\n${esc(glue)}\n` +
  (pure ? `// ---- pure.js ----\n${esc(pure)}\n` : '') +
  `// ---- app.js ----\n${esc(app)}</script>`;

html = html
  .replace('<link rel="icon" href="favicon.ico">',
    `<link rel="icon" href="data:image/x-icon;base64,${favicon.toString('base64')}">`)
  .replace('<link rel="stylesheet" href="style.css">',
    `<style>\n${css}</style>`)
  .replace(/<script src="pure\.js"><\/script>\s*/, '')
  .replace('<script type="module" src="app.js"></script>', scripts);

mkdirSync(join(root, 'dist'), { recursive: true });
const out = join(root, 'dist', 'P4GSaveEditor.html');
writeFileSync(out, html, 'utf8');
console.log(`${out} written (${html.length} bytes)`);
