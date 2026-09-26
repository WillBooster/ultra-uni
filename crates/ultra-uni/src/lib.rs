mod format;
mod language;
mod lint;
mod metrics;
mod tree;

use serde::Serialize;
use tree_sitter::{Parser, Tree};
use wasm_bindgen::prelude::*;

use crate::language::{LANGUAGE_IDS, LanguageSpec};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export type Language =
  | 'c' | 'cpp' | 'csharp' | 'css' | 'dart' | 'haskell' | 'html' | 'java' | 'javascript' | 'jsp'
  | 'kotlin' | 'php' | 'python' | 'ruby' | 'rust' | 'text' | 'tsx' | 'typescript' | 'zig';

export interface Metrics {
  lines: { total: number; code: number; comment: number; blank: number };
  /** The complexity metrics are omitted for languages without functions or control flow (CSS, HTML, JSP, and text). */
  functionCount?: number;
  cyclomaticComplexity?: number;
  cognitiveComplexity?: number;
  maxNestingDepth?: number;
}

export interface Position {
  line: number;
  /** Counted in UTF-16 code units, like JavaScript string indices. */
  column: number;
}

export interface Diagnostic {
  rule: 'syntax-error' | 'trailing-whitespace';
  message: string;
  start: Position;
  end: Position;
}
"#;

#[wasm_bindgen(js_name = supportedLanguages, unchecked_return_type = "Language[]")]
pub fn supported_languages() -> Vec<String> {
    LANGUAGE_IDS.iter().map(|&id| id.to_owned()).collect()
}

/// Parses `source` and returns its syntax tree as an S-expression.
#[wasm_bindgen(js_name = syntaxTree)]
pub fn syntax_tree(
    // Any string, as before `Language` existed, so existing typed callers still compile.
    #[wasm_bindgen(unchecked_param_type = "Language | (string & {})")] language: &str,
    source: &str,
) -> Result<String, JsError> {
    let (_, tree) = parse(language, source)?;
    let tree = tree.ok_or_else(|| JsError::new(&format!("{language} has no syntax tree")))?;
    Ok(tree.root_node().to_sexp())
}

#[wasm_bindgen(unchecked_return_type = "Metrics")]
pub fn measure(
    #[wasm_bindgen(unchecked_param_type = "Language")] language: &str,
    source: &str,
) -> Result<JsValue, JsError> {
    let (spec, tree) = parse(language, source)?;
    to_js(&metrics::measure(source, tree.as_ref(), spec.profile))
}

#[wasm_bindgen(unchecked_return_type = "Diagnostic[]")]
pub fn lint(
    #[wasm_bindgen(unchecked_param_type = "Language")] language: &str,
    source: &str,
) -> Result<JsValue, JsError> {
    let (_, tree) = parse(language, source)?;
    to_js(&lint::lint(language, source, tree.as_ref()))
}

/// Throws on syntax errors, where whitespace may belong to an unterminated literal, and returns
/// `source` unchanged when formatting would introduce one.
#[wasm_bindgen]
pub fn format(
    #[wasm_bindgen(unchecked_param_type = "Language")] language: &str,
    source: &str,
) -> Result<String, JsError> {
    let (_, tree) = parse(language, source)?;
    if tree
        .as_ref()
        .is_some_and(|tree| lint::has_syntax_errors(source, tree))
    {
        return Err(JsError::new("Cannot format code with syntax errors"));
    }
    let formatted = format::format(language, source, tree.as_ref());
    if formatted == source {
        return Ok(formatted);
    }
    // Whitespace can change a parse, as a final newline does after Ruby's `x = y&`, so the output
    // must stay free of the syntax errors `lint` reports.
    let (_, formatted_tree) = parse(language, &formatted)?;
    let breaks_syntax = formatted_tree
        .as_ref()
        .is_some_and(|tree| lint::has_syntax_errors(&formatted, tree));
    Ok(if breaks_syntax {
        source.to_owned()
    } else {
        formatted
    })
}

fn parse(language: &str, source: &str) -> Result<(LanguageSpec, Option<Tree>), JsError> {
    let spec = language::resolve(language)
        .ok_or_else(|| JsError::new(&format!("Unsupported language: {language}")))?;
    let Some(grammar) = &spec.grammar else {
        return Ok((spec, None));
    };
    let mut parser = Parser::new();
    parser.set_language(grammar)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| JsError::new("Parsing was cancelled"))?;
    Ok((spec, Some(tree)))
}

// Binds `JSON.parse` directly instead of via js-sys to keep the dependency tree small.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = JSON, js_name = parse)]
    fn parse_json(json: &str) -> JsValue;
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsError> {
    Ok(parse_json(&serde_json::to_string(value)?))
}
