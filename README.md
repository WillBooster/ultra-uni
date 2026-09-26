# ultra-uni

[![Test](https://github.com/WillBooster/ultra-uni/actions/workflows/test.yml/badge.svg)](https://github.com/WillBooster/ultra-uni/actions/workflows/test.yml)
[![Test rust](https://github.com/WillBooster/ultra-uni/actions/workflows/test-rust.yml/badge.svg)](https://github.com/WillBooster/ultra-uni/actions/workflows/test-rust.yml)
[![wbfy](https://img.shields.io/badge/wbfy-20.20.1-1e90ff.svg)](https://github.com/WillBooster/shared/tree/main/packages/wbfy)

All-in-one metrics, linter, and formatter for multiple languages, built for WebAssembly and Cloudflare Workers.

## Usage

The package imports its `.wasm` file directly, so bundle it with Wrangler for Cloudflare Workers.

```ts
import { format, lint, measure, supportedLanguages, syntaxTree } from 'ultra-uni';

measure('python', 'def f(x):\n    return 1 if x else 2\n');
// => { lines: { total: 2, code: 2, comment: 0, blank: 0 }, functionCount: 1,
//      cyclomaticComplexity: 3, cognitiveComplexity: 1, maxNestingDepth: 1 }

lint('javascript', 'const x = 1; \n');
// => [{ rule: 'trailing-whitespace', message: 'Trailing whitespace',
//       start: { line: 1, column: 13 }, end: { line: 1, column: 14 } }]

format('javascript', 'const x = 1; \n\n');
// => 'const x = 1;\n'

syntaxTree('typescript', 'const x = 1;');
// => '(program (lexical_declaration (variable_declarator name: (identifier) value: (number))))'
```

Every function throws on an unsupported language, and `syntaxTree` also throws for `text`, which has no grammar.

Supported languages (`supportedLanguages()`): `c`, `cpp`, `csharp`, `css`, `dart`, `haskell`, `html`, `java`, `javascript`, `jsp`, `kotlin`, `php`, `python`, `ruby`, `rust`, `text`, `tsx`, `typescript`, and `zig`.

### Metrics

- `lines`: a line is `blank` if it holds only whitespace, `comment` if all its tokens belong to comments, and `code` otherwise.
- `functionCount`: functions, methods, constructors, property accessors, and lambdas that have a body; every Ruby block counts as a lambda.
- `cyclomaticComplexity`: McCabe complexity of the whole file: 1, plus 1 per function, plus 1 per conditional (including Rust's `let ... else`), loop, exception handler, chained branch (`elif`, `elsif`, `elseif`), guard (e.g., `case ... when`), case other than a default or an unguarded `_` wildcard (or Haskell's `otherwise` guard), and logical or coalescing operator (`&&`, `||`, `and`, `or`, `xor`, `??`, `?:`, `orelse`).
- `cognitiveComplexity`: a simplified SonarSource cognitive complexity: 1 plus the nesting level per conditional, loop, handler, and switch (Rust's `let ... else` scores only through its `else`); 1 per `else` branch of a conditional, loop, or handler (not the `else` of a ternary expression or of a switch), chained branch (`elif`, `elsif`, `elseif`), or guard (e.g., `case ... when`); and 1 per sequence of the same logical operator. A function nested inside another function also raises the nesting level by 1.
- `maxNestingDepth`: the deepest nesting of conditionals, loops, handlers, and switches.

CSS, HTML, JSP, and text have no functions or control flow, so their complexity metrics are omitted (`undefined`).

### Linting and formatting

`lint` reports syntax errors (`syntax-error`) and trailing spaces and tabs outside value regions (`trailing-whitespace`): literals, Ruby's `__END__` data, HTML attribute values and `<pre>`, `<textarea>`, `<script>`, and `<style>` contents, the template text of JSP and PHP files, and the code embedded in JSP templates, whose inner literals are not parsed. Positions are 1-based, with columns counted in UTF-16 code units like JavaScript string indices.

`format` removes trailing whitespace and trailing blank lines outside value regions, converts CRLF line endings outside them to LF, and ends non-empty code with a newline unless a value region reaches the end of the file, since such a region may lack a closing delimiter (a Ruby heredoc whose terminator line has no newline, `__END__` data, an unclosed `<pre>`, or template text). It never changes the content of a value region. It throws when the code has syntax errors and returns the code unchanged when formatting would introduce one.

## Development

Tools are pinned in `mise.toml` (including zig and wasm-bindgen) and the Rust toolchain in `rust-toolchain.toml`, which mise also reads; run `mise install`, then `bun install` and `bun run test`.
