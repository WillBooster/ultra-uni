use tree_sitter::{Language, Parser};
use wasm_bindgen::prelude::*;

/// Parses `source` and returns its syntax tree as an S-expression.
#[wasm_bindgen(js_name = syntaxTree)]
pub fn syntax_tree(language: &str, source: &str) -> Result<String, JsError> {
    let language = resolve_language(language)
        .ok_or_else(|| JsError::new(&format!("Unsupported language: {language}")))?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| JsError::new("Parsing was cancelled"))?;
    Ok(tree.root_node().to_sexp())
}

fn resolve_language(name: &str) -> Option<Language> {
    match name {
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => None,
    }
}
