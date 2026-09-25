import path from 'node:path';

import { Miniflare } from 'miniflare';
import { afterAll, expect, test } from 'vitest';

const miniflare = new Miniflare({
  compatibilityDate: '2026-07-01',
  modules: true,
  modulesRules: [
    { type: 'ESModule', include: ['**/*.js'] },
    { type: 'CompiledWasm', include: ['**/*.wasm'] },
  ],
  scriptPath: path.resolve(import.meta.dirname, '..', 'fixtures', 'worker.js'),
});

afterAll(async () => {
  await miniflare.dispose();
});

test.each([
  {
    language: 'typescript',
    source: 'const x: number = 1;',
    tree: '(program (lexical_declaration (variable_declarator name: (identifier) type: (type_annotation (predefined_type)) value: (number))))',
  },
  {
    language: 'python',
    source: 'def f():\n    return 1\n',
    tree: '(module (function_definition name: (identifier) parameters: (parameters) body: (block (return_statement (integer)))))',
  },
  {
    language: 'tsx',
    source: 'const x = <div />;',
    tree: '(program (lexical_declaration (variable_declarator name: (identifier) value: (jsx_self_closing_element name: (identifier)))))',
  },
])('parses $language inside workerd', async ({ language, source, tree }) => {
  const response = await miniflare.dispatchFetch('http://localhost', {
    body: JSON.stringify({ language, source }),
    method: 'POST',
  });
  expect(await response.json()).toEqual({ tree });
});

test('rejects an unsupported language', async () => {
  const response = await miniflare.dispatchFetch('http://localhost', {
    body: JSON.stringify({ language: 'cobol', source: '' }),
    method: 'POST',
  });
  expect(response.status).toBe(400);
  expect(await response.json()).toEqual({ error: 'Unsupported language: cobol' });
});
