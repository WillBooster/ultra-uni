use std::ops::Range;

use tree_sitter::{Node, Tree};

/// Removes trailing whitespace (including the `\r` of CRLF) and trailing blank lines, and ends
/// non-empty code with a single newline.
pub fn format(source: &str, tree: Option<&Tree>) -> String {
    let mut formatted = String::with_capacity(source.len());
    let mut last = 0;
    for range in trailing_whitespace(source, tree) {
        formatted.push_str(&source[last..range.start]);
        last = range.end;
    }
    formatted.push_str(&source[last..]);
    let trimmed_len = formatted.trim_end_matches(['\n', ' ', '\t', '\r']).len();
    formatted.truncate(trimmed_len);
    if !formatted.is_empty() {
        formatted.push('\n');
    }
    formatted
}

/// Byte ranges of whitespace before each line break or the end of `source`, excluding whitespace
/// inside literals where it is part of the value.
pub fn trailing_whitespace(source: &str, tree: Option<&Tree>) -> Vec<Range<usize>> {
    let mut literals = Vec::new();
    if let Some(tree) = tree {
        collect_literals(tree.root_node(), &mut literals);
    }
    let mut ranges = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let end = line_start + content.len();
        let start = line_start + content.trim_end_matches([' ', '\t', '\r']).len();
        let in_literal = literals
            .iter()
            .any(|literal| literal.start <= start && end <= literal.end);
        if start < end && !in_literal {
            ranges.push(start..end);
        }
        line_start += line.len();
    }
    ranges
}

/// Collects the innermost literal nodes, so that containers such as a concatenation of strings do
/// not hide the whitespace between their parts. Single-line nodes count too because some grammars
/// split a multi-line literal into one node per line, as PHP does for heredocs.
fn collect_literals(node: Node, literals: &mut Vec<Range<usize>>) {
    let mut cursor = node.walk();
    let has_literal_child = node
        .named_children(&mut cursor)
        .any(|child| is_literal_kind(child.kind()));
    if is_literal_kind(node.kind()) && !has_literal_child {
        literals.push(node.byte_range());
        return;
    }
    for child in node.children(&mut cursor) {
        collect_literals(child, literals);
    }
}

fn is_literal_kind(kind: &str) -> bool {
    ["string", "heredoc", "nowdoc"]
        .iter()
        .any(|keyword| kind.contains(keyword))
}
