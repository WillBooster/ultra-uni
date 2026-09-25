import { execFileSync } from 'node:child_process';
import path from 'node:path';

export function setup(): void {
  execFileSync('node', [path.resolve(import.meta.dirname, '..', '..', 'scripts', 'buildWasm.mjs')], {
    stdio: 'inherit',
  });
}
