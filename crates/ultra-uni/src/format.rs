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
        collect_multiline_literals(tree.root_node(), &mut literals);
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

fn collect_multiline_literals(node: Node, literals: &mut Vec<Range<usize>>) {
    if node.start_position().row == node.end_position().row {
        return;
    }
    let kind = node.kind();
    let is_literal = ["string", "heredoc", "nowdoc"]
        .iter()
        .any(|keyword| kind.contains(keyword));
    if is_literal {
        literals.push(node.byte_range());
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_multiline_literals(child, literals);
    }
}
