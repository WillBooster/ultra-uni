use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::tree::walk;

/// Removes trailing whitespace (including the `\r` of CRLF) and trailing blank lines outside
/// literals, and ends non-empty code with a single newline unless it ends inside an open literal.
pub fn format(source: &str, tree: Option<&Tree>) -> String {
    let Literals { ranges, open_end } = literals(tree);
    // The trailing blank run stops at the end of the last literal, which can reach the end of the
    // file.
    let trimmed_end = source.trim_end_matches(['\n', ' ', '\t', '\r']).len();
    let content_end = trimmed_end.max(ranges.last().map_or(0, |range| range.end));
    let mut formatted = String::with_capacity(content_end + 1);
    let mut last = 0;
    for range in trailing_whitespace(source, &ranges) {
        if range.start >= content_end {
            break;
        }
        formatted.push_str(&source[last..range.start]);
        last = range.end;
    }
    formatted.push_str(&source[last..content_end]);
    // A newline appended at the end of an open literal would join its value.
    if !formatted.is_empty() && open_end != Some(content_end) {
        formatted.push('\n');
    }
    formatted
}

pub struct Literals {
    /// Byte ranges of the innermost literal nodes, in document order and disjoint.
    pub ranges: Vec<Range<usize>>,
    /// The end of the last literal when it has no closing delimiter and so runs to the end of the
    /// file: an unterminated Ruby heredoc, whose closing node is zero-width, or Ruby's `__END__`
    /// data.
    pub open_end: Option<usize>,
}

pub fn literals(tree: Option<&Tree>) -> Literals {
    let mut literals = Literals {
        ranges: Vec::new(),
        open_end: None,
    };
    if let Some(tree) = tree {
        collect_literals(tree.root_node(), &mut literals);
    }
    literals
}

/// Byte ranges of whitespace before each line break or the end of `source`, excluding whitespace
/// inside `literals` where it is part of the value.
pub fn trailing_whitespace(source: &str, literals: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let end = line_start + content.len();
        let mut start = line_start + content.trim_end_matches([' ', '\t', '\r']).len();
        let index = literals.partition_point(|literal| literal.end <= start);
        if let Some(literal) = literals.get(index).filter(|literal| literal.start <= start) {
            start = literal.end;
        }
        if start < end {
            ranges.push(start..end);
        }
        line_start += line.len();
    }
    ranges
}

/// Collects the innermost literal nodes, so that containers such as a concatenation of strings do
/// not hide the whitespace between their parts. Single-line nodes count too because some grammars
/// split a multi-line literal into one node per line, as PHP does for heredocs.
fn collect_literals(root: Node, literals: &mut Literals) {
    walk(root, |node, _| {
        let mut cursor = node.walk();
        let has_literal_child = node
            .named_children(&mut cursor)
            .any(|child| is_literal_kind(child.kind()));
        if is_literal_kind(node.kind()) && !has_literal_child {
            let range = node.byte_range();
            let is_open = range.is_empty() || node.kind() == "uninterpreted";
            literals.open_end = is_open.then_some(range.end);
            literals.ranges.push(range);
            return false;
        }
        true
    });
}

fn is_literal_kind(kind: &str) -> bool {
    ["string", "heredoc", "nowdoc", "quasiquote", "uninterpreted"]
        .iter()
        .any(|keyword| kind.contains(keyword))
}
