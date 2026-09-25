use serde::Serialize;
use tree_sitter::{Node, Tree};

use crate::format::trailing_whitespace;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub rule: &'static str,
    pub message: String,
    pub start: Position,
    pub end: Position,
}

/// 1-based line and column, with columns counted in UTF-16 code units like JavaScript strings.
#[derive(Serialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

pub fn lint(source: &str, tree: Option<&Tree>) -> Vec<Diagnostic> {
    let lines = LineIndex::new(source);
    let mut diagnostics = Vec::new();
    if let Some(tree) = tree {
        collect_syntax_errors(&lines, tree.root_node(), &mut diagnostics);
    }
    for range in trailing_whitespace(source, tree) {
        diagnostics.push(Diagnostic {
            rule: "trailing-whitespace",
            message: "Trailing whitespace".to_owned(),
            start: lines.position(range.start),
            end: lines.position(range.end),
        });
    }
    diagnostics.sort_by_key(|diagnostic| (diagnostic.start.line, diagnostic.start.column));
    diagnostics
}

fn collect_syntax_errors(lines: &LineIndex, node: Node, diagnostics: &mut Vec<Diagnostic>) {
    if !node.has_error() {
        return;
    }
    let message = if node.is_error() {
        "Unexpected syntax".to_owned()
    } else if node.is_missing() {
        format!("Missing {}", node.kind())
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_syntax_errors(lines, child, diagnostics);
        }
        return;
    };
    diagnostics.push(Diagnostic {
        rule: "syntax-error",
        message,
        start: lines.position(node.start_byte()),
        end: lines.position(node.end_byte()),
    });
}

struct LineIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    fn new(source: &'a str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Self {
            source,
            line_starts,
        }
    }

    fn position(&self, byte: usize) -> Position {
        let line = self.line_starts.partition_point(|&start| start <= byte) - 1;
        let column = self.source[self.line_starts[line]..byte]
            .encode_utf16()
            .count();
        Position {
            line: line as u32 + 1,
            column: column as u32 + 1,
        }
    }
}
