use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::tree::walk;

/// Removes trailing whitespace (including the `\r` of CRLF) and trailing blank lines outside
/// literals, and ends non-empty code with a single newline unless it ends inside an open literal.
pub fn format(source: &str, tree: Option<&Tree>) -> String {
    let Literals { ranges, open_end } = literals(source, tree);
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
    if !formatted.is_empty() && open_end != Some(content_end) && !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}

pub struct Literals {
    /// Byte ranges of the innermost literal nodes and escape sequences, merged where they overlap
    /// or touch, since a grammar may split one value into several pieces (Ruby's escaped space is
    /// an `escape_sequence` between two contents). Sorted and disjoint.
    pub ranges: Vec<Range<usize>>,
    /// The end of the last literal when it has no closing delimiter and so runs to the end of the
    /// file: an unterminated Ruby heredoc, whose closing node is zero-width, or Ruby's `__END__`
    /// data.
    pub open_end: Option<usize>,
}

pub fn literals(source: &str, tree: Option<&Tree>) -> Literals {
    let mut literals = Literals {
        ranges: Vec::new(),
        open_end: None,
    };
    if let Some(tree) = tree {
        collect_literals(source, tree.root_node(), &mut literals);
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
fn collect_literals(source: &str, root: Node, literals: &mut Literals) {
    let mut ranges = Vec::new();
    walk(root, |node, _| {
        if node.kind().contains("escape_sequence") || is_preformatted_element(source, node) {
            ranges.push(node.byte_range());
            return false;
        }
        let mut cursor = node.walk();
        let has_literal_child = node
            .named_children(&mut cursor)
            .any(|child| is_literal_kind(child.kind()));
        if is_literal_kind(node.kind()) && !has_literal_child {
            let range = node.byte_range();
            let is_open = range.is_empty() || node.kind() == "uninterpreted";
            literals.open_end = is_open.then_some(range.end);
            ranges.push(range);
            return false;
        }
        true
    });
    ranges.sort_by_key(|range| range.start);
    for range in ranges {
        match literals.ranges.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => literals.ranges.push(range),
        }
    }
}

/// An HTML `<pre>` or `<textarea>` element, whose text keeps its whitespace; the grammar's `text`
/// nodes exclude the whitespace at their edges.
fn is_preformatted_element(source: &str, node: Node) -> bool {
    node.kind() == "element"
        && node
            .child(0)
            .and_then(|start_tag| start_tag.named_child(0))
            .filter(|name| name.kind() == "tag_name")
            .is_some_and(|name| {
                let name = &source[name.byte_range()];
                name.eq_ignore_ascii_case("pre") || name.eq_ignore_ascii_case("textarea")
            })
}

fn is_literal_kind(kind: &str) -> bool {
    ["string", "heredoc", "nowdoc", "quasiquote", "uninterpreted"]
        .iter()
        .any(|keyword| kind.contains(keyword))
        // Markup text, attribute values, and embedded code in HTML, JSP (embedded-template), and PHP
        // templates, whose inner literals no grammar models.
        || matches!(
            kind,
            "text" | "raw_text" | "attribute_value" | "quoted_attribute_value" | "code" | "content"
        )
}
