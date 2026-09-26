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

test.each([
  {
    construct: 'a guarded C# discard case',
    language: 'csharp',
    source: 'class A { int F(int x) { switch (x) { case _ when x > 0: return 1; default: return 2; } } }\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a guarded Python wildcard case',
    language: 'python',
    source: 'def f(x):\n    match x:\n        case _ if x:\n            pass\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a guarded Rust wildcard arm',
    language: 'rust',
    source: 'fn f(x: i32) -> i32 { match x { _ if x > 0 => 1, _ => 0 } }\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a guarded Dart wildcard case',
    language: 'dart',
    source: 'int f(int x) { switch (x) { case _ when x > 0: return 1; default: return 2; } }\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a guarded Haskell wildcard alternative',
    language: 'haskell',
    source: 'f x = case x of\n  _ | x > 0 -> 1\n  _ -> 0\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a Dart if-case guard',
    language: 'dart',
    source: 'int f(int x) { if (x case int n when n > 0) { return 1; } return 0; }\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a Haskell lambda-cases expression',
    language: 'haskell',
    source: 'f = \\cases\n  1 -> 1\n  2 -> 2\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a C# exception filter',
    language: 'csharp',
    source: 'class A { int F(int x) { try { return 1; } catch (System.Exception e) when (x > 0) { return 2; } } }\n',
    cyclomaticComplexity: 4,
  },
  {
    construct: 'a Rust let-else',
    language: 'rust',
    source: 'fn f() { let Some(x) = opt else { return }; }\n',
    cyclomaticComplexity: 3,
  },
  {
    construct: "Kotlin's elvis operator",
    language: 'kotlin',
    source: 'fun f(a: Int?, b: Int): Int = a ?: b\n',
    cyclomaticComplexity: 3,
  },
  {
    construct: "Zig's orelse",
    language: 'zig',
    source: 'fn f(a: ?i32) i32 {\n    return a orelse 0;\n}\n',
    cyclomaticComplexity: 3,
  },
])('scores $construct', async ({ cyclomaticComplexity, language, source }) => {
  const { body } = await call('measure', language, source);
  expect(body).toMatchObject({ result: { cyclomaticComplexity } });
});

test.each([
  { construct: 'destructor', source: 'class A { ~A() { } }\n' },
  { construct: 'expression-bodied property', source: 'class A { int P => 1; int Q { get; set; } = 2; }\n' },
  { construct: 'expression-bodied indexer', source: 'class A { int this[int i] => i; }\n' },
])('counts a C# $construct as a function', async ({ source }) => {
  const { body } = await call('measure', 'csharp', source);
  expect(body).toMatchObject({ result: { functionCount: 1, cyclomaticComplexity: 2 } });
});

test('scores each non-default Haskell guard clause as a guard', async () => {
  const { body } = await call('measure', 'haskell', 'f x\n  | x > 0 = 1\n  | x > 1 = 2\n  | otherwise = 3\n');
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 4, cognitiveComplexity: 2 } });
});

test.each([
  { construct: 'a C++ rvalue reference', language: 'cpp', source: 'void f(int&& x) {}\n', cyclomaticComplexity: 2 },
  {
    construct: 'a Haskell operator section',
    language: 'haskell',
    source: 'f = foldr (&&) True\n',
    cyclomaticComplexity: 1,
  },
])('does not score $construct as a logical operator', async ({ cyclomaticComplexity, language, source }) => {
  const { body } = await call('measure', language, source);
  expect(body).toMatchObject({ result: { cyclomaticComplexity, cognitiveComplexity: 0 } });
});

test('scores Python comprehension clauses as branches', async () => {
  const { body } = await call('measure', 'python', 'ys = [x for x in xs if x]\n');
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 3, cognitiveComplexity: 2, maxNestingDepth: 1 } });
});

test('scores Haskell comprehension qualifiers as branches', async () => {
  const { body } = await call('measure', 'haskell', 'ys = [x | x <- xs, x > 0]\n');
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 3, cognitiveComplexity: 2, maxNestingDepth: 1 } });
});

test('scores C# query clauses as branches', async () => {
  const { body } = await call(
    'measure',
    'csharp',
    'class A { int[] F(int[] xs) => (from x in xs where x > 0 select x).ToArray(); }\n'
  );
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 4, cognitiveComplexity: 2, maxNestingDepth: 1 } });
});

test('counts a PHP property hook with a body as a function', async () => {
  const { body } = await call('measure', 'php', '<?php class A { public int $x { get => 1; } }\n');
  expect(body).toMatchObject({ result: { functionCount: 1, cyclomaticComplexity: 2 } });
});

test('does not score the fallback of a Ruby case as an else branch', async () => {
  const { body } = await call('measure', 'ruby', 'def f(x)\n  case x\n  when 1\n    1\n  else\n    2\n  end\nend\n');
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 3, cognitiveComplexity: 1 } });
});

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

// Only the non-wildcard arm adds a path besides the file and the function.
test.each([
  { language: 'rust', source: 'fn f(x: i32) -> i32 { match x { 1 => 1, _ => 0 } }\n' },
  {
    language: 'python',
    source: 'def f(x):\n    match x:\n        case 1:\n            pass\n        case _:\n            pass\n',
  },
  { language: 'haskell', source: 'f x = case x of\n  1 -> 1\n  _ -> 0\n' },
])('does not count a wildcard case as a path in $language', async ({ language, source }) => {
  const { body } = await call('measure', language, source);
  expect(body).toMatchObject({ result: { cyclomaticComplexity: 3 } });
});

test('counts a Ruby lambda once through its block', async () => {
  const { body } = await call('measure', 'ruby', 'f = ->(x) { x }\n');
  expect(body).toMatchObject({ result: { functionCount: 1, cyclomaticComplexity: 2 } });
});

test.each([
  {
    construct: 'C# auto-property and abstract method',
    language: 'csharp',
    source: 'abstract class A { public int X { get; set; } abstract int G(); }\n',
  },
  {
    construct: 'PHP interface property hook',
    language: 'php',
    source: '<?php interface I { public int $x { get; } }\n',
  },
  { construct: 'Kotlin interface function', language: 'kotlin', source: 'interface A {\n  fun g(): Int\n}\n' },
])('does not count a bodiless $construct as a function', async ({ language, source }) => {
  const { body } = await call('measure', language, source);
  expect(body).toMatchObject({ result: { functionCount: 0, cyclomaticComplexity: 1 } });
});

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

test('formats trailing whitespace between the parts of a string concatenation', async () => {
  const { body } = await call('format', 'python', 'x = ("a"   \n     "b")\n');
  expect(body).toEqual({ result: 'x = ("a"\n     "b")\n' });
});

test.each([
  { language: 'haskell', source: 's = [r|\nhello   \nworld|]\n' },
  { language: 'kotlin', source: 'val s = """a  \nb"""\n' },
  { language: 'php', source: '<?php\n$s = <<<EOT\n  a  \nEOT;\n' },
  { language: 'ruby', source: 'x = <<~EOS\n  a  \nEOS\n' },
  { language: 'csharp', source: 'var s = """\n  a  \n  """;\n' },
  { language: 'rust', source: 'fn f() {\n    let s = r"a  \nb";\n}\n' },
])('keeps trailing whitespace inside a $language multi-line literal', async ({ language, source }) => {
  const { body } = await call('format', language, source);
  expect(body).toEqual({ result: source });
});

test.each([
  { construct: 'unterminated heredoc with trailing whitespace', source: 's = <<~EOS\n  abc   \n' },
  { construct: 'unterminated heredoc without a final newline', source: 's = <<~EOS\n  abc' },
  { construct: '__END__ data section', source: 'puts DATA.read\n__END__\ndata line  \r\nmore\n\n' },
])('keeps a Ruby $construct that reaches the end of the file', async ({ source }) => {
  expect(await call('lint', 'ruby', source)).toEqual({ status: 200, body: { result: [] } });
  expect(await call('format', 'ruby', source)).toEqual({ status: 200, body: { result: source } });
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

test.each([
  { construct: 'property with a type', source: 'class A { val a: Int = 1 }\n' },
  { construct: 'property without a type', source: 'class A { val x = 1 }\n' },
  { construct: 'initializer', source: 'class A { init { } }\n' },
])('formats a Kotlin one-line class body with a $construct that lint reports as clean', async ({ source }) => {
  expect(await call('lint', 'kotlin', source)).toEqual({ status: 200, body: { result: [] } });
  expect(await call('format', 'kotlin', source)).toEqual({ status: 200, body: { result: source } });
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

test('handles deeply nested code without breaking later calls', async () => {
  const source = `let x = ${'['.repeat(20_000)}${']'.repeat(20_000)};\n`;
  for (const operation of ['measure', 'lint', 'format']) {
    const { status } = await call(operation, 'javascript', source);
    expect(status).toBe(200);
  }
  const { body } = await call('measure', 'text', 'ok\n');
  expect(body).toEqual({ result: { lines: { total: 1, code: 1, comment: 0, blank: 0 } } });
});

test('rejects an unsupported language', async () => {
  expect(await call('measure', 'cobol', '')).toEqual({
    status: 400,
    body: { error: 'Unsupported language: cobol' },
  });
});
