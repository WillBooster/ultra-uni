# ultra-uni

[![wbfy](https://img.shields.io/badge/wbfy-20.20.1-1e90ff.svg)](https://github.com/WillBooster/shared/tree/main/packages/wbfy)

All-in-one metrics, linter, and formatter for multiple languages, built for WebAssembly and Cloudflare Workers.

## Usage

The package imports its `.wasm` file directly, so bundle it with Wrangler for Cloudflare Workers.

```ts
import { syntaxTree } from 'ultra-uni';

syntaxTree('typescript', 'const x = 1;');
// => '(program (lexical_declaration (variable_declarator name: (identifier) value: (number))))'
```

Supported languages: `typescript`, `tsx`, and `python`.

## Development

Tools are pinned in `mise.toml` (including Rust, zig, and wasm-bindgen); run `mise install`, then `bun install` and `bun run test`.
