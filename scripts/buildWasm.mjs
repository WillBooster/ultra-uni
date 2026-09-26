import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const target = 'wasm32-unknown-unknown';
const rootDir = path.resolve(import.meta.dirname, '..');
const distDir = path.join(rootDir, 'dist');

// Grammar crates compile their C parsers with cc-rs, which needs a C compiler and archiver for
// wasm plus a libc; zig provides the former, and tree-sitter-language ships the libc headers
// that tree-sitter's own wasm stdlib implements.
const metadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--filter-platform', target], {
    cwd: rootDir,
    encoding: 'utf8',
  })
);
const languageCrate = metadata.packages.find((pkg) => pkg.name === 'tree-sitter-language');
const wasmHeadersDir = path.join(path.dirname(languageCrate.manifest_path), 'wasm', 'include');

execFileSync('cargo', ['build', '--release', '--target', target, '--package', 'ultra-uni'], {
  cwd: rootDir,
  env: {
    ...process.env,
    AR_wasm32_unknown_unknown: path.join(rootDir, 'scripts', 'zigar'),
    CC_wasm32_unknown_unknown: path.join(rootDir, 'scripts', 'zigcc'),
    // Those libc headers omit declarations that a full libc's `<wctype.h>` and `<assert.h>` expose
    // and tree-sitter-cpp's scanner uses: `wchar_t` (from the compiler's `<stddef.h>`) and C11's
    // `static_assert`.
    CFLAGS_wasm32_unknown_unknown: `-I${wasmHeadersDir} -include stddef.h -Dstatic_assert=_Static_assert`,
  },
  stdio: 'inherit',
});

fs.rmSync(distDir, { force: true, recursive: true });
execFileSync(
  'wasm-bindgen',
  [
    '--target',
    'web',
    '--out-dir',
    distDir,
    '--out-name',
    'ultra_uni',
    path.join(metadata.target_directory, target, 'release', 'ultra_uni.wasm'),
  ],
  { stdio: 'inherit' }
);

// Wrangler turns a `.wasm` import into a compiled WebAssembly.Module, so the entry point
// instantiates it synchronously at import time.
const exports = `export { format, lint, measure, supportedLanguages, syntaxTree } from './ultra_uni.js';\n`;
fs.writeFileSync(
  path.join(distDir, 'index.js'),
  `import wasmModule from './ultra_uni_bg.wasm';
import { initSync } from './ultra_uni.js';

initSync({ module: wasmModule });

${exports}`
);
fs.writeFileSync(
  path.join(distDir, 'index.d.ts'),
  `${exports}export type { Diagnostic, Language, Metrics, Position } from './ultra_uni.js';\n`
);
