use serde::Serialize;
use tree_sitter::{Node, Tree};

use crate::format::{trailing_whitespace, value_ranges};
use crate::tree::walk;

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
    for mut range in trailing_whitespace(source, &value_ranges(source, tree)) {
        // `format` converts CRLF, but a CRLF line ending alone is not trailing whitespace.
        if source[range.clone()].ends_with('\r') {
            range.end -= 1;
        }
        if range.is_empty() {
            continue;
        }
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

/// Whether `lint` reports a `syntax-error`. `Node::has_error` also counts invisible zero-width
/// MISSING tokens that tree-sitter-kotlin-ng inserts into valid one-line class bodies.
pub fn has_syntax_errors(source: &str, tree: &Tree) -> bool {
    let mut diagnostics = Vec::new();
    collect_syntax_errors(&LineIndex::new(source), tree.root_node(), &mut diagnostics);
    !diagnostics.is_empty()
}

fn collect_syntax_errors(lines: &LineIndex, root: Node, diagnostics: &mut Vec<Diagnostic>) {
    walk(root, |node, _| {
        if !node.has_error() || is_recovered_kotlin_member(node) {
            return false;
        }
        let message = if node.is_error() {
            "Unexpected syntax".to_owned()
        } else if node.is_missing() {
            format!("Missing {}", node.kind())
        } else {
            return true;
        };
        diagnostics.push(Diagnostic {
            rule: "syntax-error",
            message,
            start: lines.position(node.start_byte()),
            end: lines.position(node.end_byte()),
        });
        false
    });
}

/// tree-sitter-kotlin-ng parses a valid one-line class body such as `class A { val x = 1 }` as an
/// `enum_class_body` whose member sits in an ERROR node, although the member itself parses cleanly.
fn is_recovered_kotlin_member(node: Node) -> bool {
    let mut cursor = node.walk();
    node.is_error()
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "enum_class_body")
        && node
            .children(&mut cursor)
            .all(|child| child.is_named() && !child.has_error())
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
