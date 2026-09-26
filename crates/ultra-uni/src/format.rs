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
    /// Byte ranges of the outermost literal nodes, in document order and disjoint.
    pub ranges: Vec<Range<usize>>,
    /// The end of the last value region when it has no closing node and so runs to the end of the
    /// file.
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

/// Kinds that join separate literals, whose parts are protected one by one because the whitespace
/// between them is code.
const CONCATENATIONS: &[&str] = &["concatenated_string", "chained_string", "string_array"];

/// Collects the outermost literal nodes, because a grammar can leave value bytes outside every
/// child node, such as the leading whitespace of a Rust raw string or a whitespace-only PHP heredoc
/// body.
fn collect_literals(source: &str, root: Node, literals: &mut Literals) {
    walk(root, |node, _| {
        let kind = node.kind();
        let is_value = is_literal_kind(kind) && !CONCATENATIONS.contains(&kind)
            // An escaped space is a value byte even where no value node encloses it.
            || kind.contains("escape_sequence")
            || is_preformatted_element(source, node);
        if !is_value {
            return true;
        }
        let mut range = node.byte_range();
        // The grammar ends an element without an end tag at its last child, before the whitespace
        // that is still its content. A value region is open when its content runs to the end of
        // the file without a closing node: such an element, an unterminated Ruby heredoc (whose
        // closing node is zero-width), or Ruby's `__END__` data.
        let mut cursor = node.walk();
        let is_unclosed_element = kind == "element"
            && !node
                .children(&mut cursor)
                .any(|child| child.kind() == "end_tag");
        if is_unclosed_element {
            range.end += source[range.end..]
                .bytes()
                .take_while(u8::is_ascii_whitespace)
                .count();
        }
        let has_open_end = is_unclosed_element && range.end == source.len()
            || kind == "uninterpreted"
            || node
                .child(node.child_count().saturating_sub(1))
                .is_some_and(|last| last.byte_range().is_empty())
            || range.is_empty();
        literals.open_end = has_open_end.then_some(range.end);
        literals.ranges.push(range);
        false
    });
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
    ["string", "heredoc", "nowdoc", "quasiquote", "uninterpreted", "regex", "subshell"]
        .iter()
        .any(|keyword| kind.contains(keyword))
        // Markup text, attribute values, and embedded code in HTML, JSP (embedded-template), and PHP
        // templates, whose inner literals no grammar models.
        || matches!(
            kind,
            "text" | "raw_text" | "attribute_value" | "quoted_attribute_value" | "code" | "content"
        )
}
