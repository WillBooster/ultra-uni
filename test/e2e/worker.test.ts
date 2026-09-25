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

async function call(operation: string, language: string, source: string): Promise<{ status: number; body: unknown }> {
  const response = await miniflare.dispatchFetch('http://localhost', {
    body: JSON.stringify({ language, operation, source }),
    method: 'POST',
  });
  return { status: response.status, body: await response.json() };
}

// The same program in each language: a function whose `if (x > 0 && x < 10)` holds a loop and is
// followed by `else if` and `else`.
const equivalentPrograms = {
  c: `// comment
int f(int x) {
  if (x > 0 && x < 10) {
    for (int i = 0; i < x; i++) {}
  } else if (x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  cpp: `// comment
int f(int x) {
  if (x > 0 && x < 10) {
    for (int i : v) {}
  } else if (x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  csharp: `// comment
class A {
  int F(int x) {
    if (x > 0 && x < 10) {
      foreach (var i in v) {}
    } else if (x < 0) {
      return 1;
    } else {
      return 2;
    }
    return 0;
  }
}
`,
  dart: `// comment
int f(int x) {
  if (x > 0 && x < 10) {
    for (var i in v) {}
  } else if (x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  java: `// comment
class A {
  int f(int x) {
    if (x > 0 && x < 10) {
      for (int i : v) {}
    } else if (x < 0) {
      return 1;
    } else {
      return 2;
    }
    return 0;
  }
}
`,
  javascript: `// comment
function f(x) {
  if (x > 0 && x < 10) {
    for (const i of v) {}
  } else if (x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  kotlin: `// comment
fun f(x: Int): Int {
  if (x > 0 && x < 10) {
    for (i in v) {}
  } else if (x < 0) {
    return 1
  } else {
    return 2
  }
  return 0
}
`,
  php: `<?php
// comment
function f($x) {
  if ($x > 0 && $x < 10) {
    foreach ($v as $i) {}
  } elseif ($x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  python: `# comment
def f(x):
    if x > 0 and x < 10:
        for i in v:
            pass
    elif x < 0:
        return 1
    else:
        return 2
    return 0
`,
  ruby: `# comment
def f(x)
  if x > 0 && x < 10
    for i in v
    end
  elsif x < 0
    return 1
  else
    return 2
  end
  0
end
`,
  rust: `// comment
fn f(x: i32) -> i32 {
    if x > 0 && x < 10 {
        for i in v {}
    } else if x < 0 {
        return 1;
    } else {
        return 2;
    }
    0
}
`,
  tsx: `// comment
function f(x: number) {
  if (x > 0 && x < 10) {
    for (const i of v) {}
  } else if (x < 0) {
    return <a />;
  } else {
    return 2;
  }
  return 0;
}
`,
  typescript: `// comment
function f(x: number): number {
  if (x > 0 && x < 10) {
    for (const i of v) {}
  } else if (x < 0) {
    return 1;
  } else {
    return 2;
  }
  return 0;
}
`,
  zig: `// comment
fn f(x: i32) i32 {
    if (x > 0 and x < 10) {
        for (v) |i| {}
    } else if (x < 0) {
        return 1;
    } else {
        return 2;
    }
    return 0;
}
`,
};

test('lists every Exercode language plus tsx', async () => {
  const response = await miniflare.dispatchFetch('http://localhost');
  expect(await response.json()).toEqual({
    languages: [
      'c',
      'cpp',
      'csharp',
      'css',
      'dart',
      'haskell',
      'html',
      'java',
      'javascript',
      'jsp',
      'kotlin',
      'php',
      'python',
      'ruby',
      'rust',
      'text',
      'tsx',
      'typescript',
      'zig',
    ],
  });
});

test.each(Object.entries(equivalentPrograms))(
  'measures %s consistently with other languages',
  async (language, source) => {
    const lineCount = source.split('\n').length - 1;
    expect(await call('measure', language, source)).toEqual({
      status: 200,
      body: {
        result: {
          lines: { total: lineCount, code: lineCount - 1, comment: 1, blank: 0 },
          functionCount: 1,
          // The file, the function, `if`, `&&`, the loop, and `else if`.
          cyclomaticComplexity: 6,
          // `if`, `&&`, the loop nested once, `else`, and `else`.
          cognitiveComplexity: 6,
          maxNestingDepth: 2,
        },
      },
    });
  }
);

test('measures Haskell', async () => {
  const source = `-- comment
f :: Int -> Int
f x = if x > 0 && x < 10 then 1 else if x < 0 then 2 else 3
`;
  const { body } = await call('measure', 'haskell', source);
  expect(body).toEqual({
    result: {
      lines: { total: 3, code: 2, comment: 1, blank: 0 },
      functionCount: 1,
      cyclomaticComplexity: 5,
      cognitiveComplexity: 4,
      maxNestingDepth: 1,
    },
  });
});

test.each([
  {
    language: 'rust',
    source: 'fn f(x: i32) -> i32 { match x { 1 => 1, _ => 0 } }\n',
    functionCount: 1,
    cyclomaticComplexity: 3,
  },
  {
    language: 'python',
    source: 'def f(x):\n    match x:\n        case 1:\n            pass\n        case _:\n            pass\n',
    functionCount: 1,
    cyclomaticComplexity: 3,
  },
  { language: 'haskell', source: 'f x = case x of\n  1 -> 1\n  _ -> 0\n', functionCount: 1, cyclomaticComplexity: 3 },
  { language: 'ruby', source: 'f = ->(x) { x }\n', functionCount: 1, cyclomaticComplexity: 2 },
  { language: 'cpp', source: 'void f(int&& x) {}\n', functionCount: 1, cyclomaticComplexity: 2 },
  {
    language: 'csharp',
    source: 'abstract class A { public int X { get; set; } abstract int G(); }\n',
    functionCount: 0,
    cyclomaticComplexity: 1,
  },
  {
    language: 'kotlin',
    source: 'interface A {\n  fun g(): Int\n}\n',
    functionCount: 0,
    cyclomaticComplexity: 1,
  },
])(
  'counts neither wildcard cases nor bodiless or duplicate functions in $language',
  async ({ cyclomaticComplexity, functionCount, language, source }) => {
    const { body } = await call('measure', language, source);
    expect(body).toMatchObject({ result: { cyclomaticComplexity, functionCount } });
  }
);

// Complexity metrics are omitted.
test.each([
  {
    language: 'css',
    source: '/* comment */\na {\n  color: red;\n}\n',
    lines: { total: 4, code: 3, comment: 1, blank: 0 },
  },
  {
    language: 'html',
    source: '<!-- comment -->\n<p>\n  hi\n</p>\n',
    lines: { total: 4, code: 3, comment: 1, blank: 0 },
  },
  {
    language: 'jsp',
    source: '<%-- comment --%>\n<p>\n  <%= x %>\n</p>\n',
    lines: { total: 4, code: 3, comment: 1, blank: 0 },
  },
  { language: 'text', source: 'hello\n\nworld\n', lines: { total: 3, code: 2, comment: 0, blank: 1 } },
])('measures only lines of $language', async ({ language, lines, source }) => {
  const { body } = await call('measure', language, source);
  expect(body).toEqual({ result: { lines } });
});

test('reports syntax errors and trailing whitespace outside literals', async () => {
  const source = 'const a = `x  \ny`;  \nconst b = ;\n';
  const { body } = await call('lint', 'javascript', source);
  expect(body).toEqual({
    result: [
      {
        rule: 'trailing-whitespace',
        message: 'Trailing whitespace',
        start: { line: 2, column: 4 },
        end: { line: 2, column: 6 },
      },
      {
        rule: 'syntax-error',
        message: 'Unexpected syntax',
        start: { line: 3, column: 9 },
        end: { line: 3, column: 10 },
      },
    ],
  });
});

test('does not report CRLF line endings as trailing whitespace', async () => {
  const { body } = await call('lint', 'text', 'a\r\nb \r\n');
  expect(body).toEqual({
    result: [
      {
        rule: 'trailing-whitespace',
        message: 'Trailing whitespace',
        start: { line: 2, column: 2 },
        end: { line: 2, column: 3 },
      },
    ],
  });
});

test('counts columns in UTF-16 code units', async () => {
  const { body } = await call('lint', 'text', 'é😀 \n');
  expect(body).toEqual({
    result: [
      {
        rule: 'trailing-whitespace',
        message: 'Trailing whitespace',
        start: { line: 1, column: 4 },
        end: { line: 1, column: 5 },
      },
    ],
  });
});

test('formats trailing whitespace in HTML script bodies', async () => {
  const { body } = await call('format', 'html', '<script>\nvar x = 1;   \n</script>\n');
  expect(body).toEqual({ result: '<script>\nvar x = 1;\n</script>\n' });
});

test('keeps trailing whitespace that ends a multi-line literal', async () => {
  const source = 'const a =\n    \\\\hello  \n    \\\\world  \n;\n';
  const { body } = await call('format', 'zig', source);
  expect(body).toEqual({ result: source });
});

test('formats trailing whitespace and blank lines, keeping them inside literals', async () => {
  const source = 's = """a  \nb"""  \r\nt = 1\t\n\n\n';
  const { body } = await call('format', 'python', source);
  expect(body).toEqual({ result: 's = """a  \nb"""\nt = 1\n' });
});

test('refuses to format code with syntax errors', async () => {
  expect(await call('format', 'python', 'def f(:\n')).toEqual({
    status: 400,
    body: { error: 'Cannot format code with syntax errors' },
  });
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
  const { body } = await call('syntaxTree', language, source);
  expect(body).toEqual({ result: tree });
});

test('rejects an unsupported language', async () => {
  expect(await call('measure', 'cobol', '')).toEqual({
    status: 400,
    body: { error: 'Unsupported language: cobol' },
  });
});
