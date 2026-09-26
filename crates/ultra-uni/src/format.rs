use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::tree::walk;

/// Removes trailing whitespace (including the `\r` of CRLF) and trailing blank lines outside
/// value regions, and ends non-empty code with a single newline unless it ends inside one.
pub fn format(source: &str, tree: Option<&Tree>) -> String {
    let ranges = value_ranges(source, tree);
    // A value region can reach the end of the file, where the trailing blank run then stops.
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
    // A value region reaching the end of the file may lack a closing delimiter (an unterminated
    // heredoc, `__END__` data, an unclosed `<pre>`, template text), so a newline there could join
    // its value.
    let ends_in_value = ranges.last().is_some_and(|range| range.end == source.len());
    if !formatted.is_empty() && !ends_in_value && !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}

/// Byte ranges of the outermost value regions, in document order and disjoint.
pub fn value_ranges(source: &str, tree: Option<&Tree>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    if let Some(tree) = tree {
        collect_value_ranges(source, tree.root_node(), &mut ranges);
    }
    ranges
}

/// Byte ranges of whitespace before each line break or the end of `source`, excluding whitespace
/// inside `values` where it is part of the value.
pub fn trailing_whitespace(source: &str, values: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let end = line_start + content.len();
        let mut start = line_start + content.trim_end_matches([' ', '\t', '\r']).len();
        let index = values.partition_point(|value| value.end <= start);
        if let Some(value) = values.get(index).filter(|value| value.start <= start) {
            start = value.end;
        }
        if start < end {
            ranges.push(start..end);
        }
        line_start += line.len();
    }
    ranges
}

/// Whether the node joins separate literals, whose parts are protected one by one because the
/// whitespace between them is code. Dart puts adjacent literals in one `string_literal` whose named
/// children are all quoted literals, while other grammars' `string_literal` holds fragments.
fn is_concatenation(node: Node) -> bool {
    let mut cursor = node.walk();
    matches!(
        node.kind(),
        "concatenated_string" | "chained_string" | "string_array"
    ) || node.named_child_count() > 1
        && node
            .named_children(&mut cursor)
            .all(|child| child.kind().contains("string_literal"))
}

/// Collects the outermost value nodes, because a grammar can leave value bytes outside every
/// child node, such as the leading whitespace of a Rust raw string or a whitespace-only PHP heredoc
/// body.
fn collect_value_ranges(source: &str, root: Node, ranges: &mut Vec<Range<usize>>) {
    walk(root, |node, _| {
        let kind = node.kind();
        let is_value = is_literal_kind(kind) && !is_concatenation(node)
            // An escaped space is a value byte even where no value node encloses it.
            || kind.contains("escape_sequence")
            || is_preformatted_element(source, node);
        if !is_value {
            return true;
        }
        let mut range = node.byte_range();
        // The grammar ends an element without an end tag at its last child, before the whitespace
        // that is still its content.
        let mut cursor = node.walk();
        if kind == "element"
            && !node
                .children(&mut cursor)
                .any(|child| child.kind() == "end_tag")
        {
            range.end += source[range.end..]
                .bytes()
                .take_while(u8::is_ascii_whitespace)
                .count();
        }
        ranges.push(range);
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
