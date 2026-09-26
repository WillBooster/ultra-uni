use tree_sitter::Node;

/// Visits `root` and its descendants in preorder, descending into a node's children only when
/// `visit` returns `true`. It iterates instead of recursing because deeply nested input would
/// otherwise overflow the wasm stack, and a trap leaves the instance unusable for later calls.
pub fn walk<'tree>(root: Node<'tree>, mut visit: impl FnMut(Node<'tree>, usize) -> bool) {
    let mut cursor = root.walk();
    let mut depth = 0;
    loop {
        if visit(cursor.node(), depth) && cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if depth == 0 {
                return;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            cursor.goto_parent();
            depth -= 1;
        }
    }
}

/// The nearest earlier sibling that is not an extra such as a comment, which is not code.
pub fn prev_code_sibling(node: Node) -> Option<Node> {
    std::iter::successors(node.prev_sibling(), Node::prev_sibling)
        .find(|sibling| !sibling.is_extra())
}

/// The nearest later sibling that is not an extra such as a comment, which is not code.
pub fn next_code_sibling(node: Node) -> Option<Node> {
    std::iter::successors(node.next_sibling(), Node::next_sibling)
        .find(|sibling| !sibling.is_extra())
}
