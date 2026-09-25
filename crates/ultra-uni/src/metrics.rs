use serde::Serialize;
use tree_sitter::{Node, Tree};

use crate::language::Profile;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub lines: LineMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyclomatic_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognitive_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nesting_depth: Option<u32>,
}

#[derive(Serialize)]
pub struct LineMetrics {
    pub total: u32,
    pub code: u32,
    pub comment: u32,
    pub blank: u32,
}

pub fn measure(source: &str, tree: Option<&Tree>, profile: Option<&Profile>) -> Metrics {
    let lines = measure_lines(source, tree);
    let (Some(tree), Some(profile)) = (tree, profile) else {
        return Metrics {
            lines,
            function_count: None,
            cyclomatic_complexity: None,
            cognitive_complexity: None,
            max_nesting_depth: None,
        };
    };
    let mut counter = ComplexityCounter {
        profile,
        function_count: 0,
        // The file itself is one path even without any function.
        cyclomatic: 1,
        cognitive: 0,
        max_nesting_depth: 0,
    };
    counter.visit(tree.root_node(), Nesting::default());
    Metrics {
        lines,
        function_count: Some(counter.function_count),
        cyclomatic_complexity: Some(counter.cyclomatic),
        cognitive_complexity: Some(counter.cognitive),
        max_nesting_depth: Some(counter.max_nesting_depth),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum LineKind {
    Blank,
    Comment,
    Code,
}

/// A line is blank when it holds only whitespace, a comment line when every token on it is part of
/// a comment, and a code line otherwise.
fn measure_lines(source: &str, tree: Option<&Tree>) -> LineMetrics {
    let mut kinds: Vec<LineKind> = source.lines().map(|_| LineKind::Blank).collect();
    let is_blank: Vec<bool> = source.lines().map(|line| line.trim().is_empty()).collect();
    match tree {
        Some(tree) => mark_lines(source, tree.root_node(), &mut kinds),
        None => kinds.fill(LineKind::Code),
    }
    let mut metrics = LineMetrics {
        total: kinds.len() as u32,
        code: 0,
        comment: 0,
        blank: 0,
    };
    for (kind, blank) in kinds.into_iter().zip(is_blank) {
        match kind {
            _ if blank => metrics.blank += 1,
            LineKind::Code => metrics.code += 1,
            LineKind::Comment => metrics.comment += 1,
            // Whitespace-only text between tokens, e.g., inside a multi-line string's delimiters.
            LineKind::Blank => metrics.code += 1,
        }
    }
    metrics
}

fn mark_lines(source: &str, node: Node, kinds: &mut [LineKind]) {
    let is_comment = node.is_named()
        && match node.kind() {
            "haddock" => true,
            // The embedded-template grammar used for JSP parses `<%-- --%>` as a directive.
            "directive" | "output_directive" => source[node.start_byte()..].starts_with("<%--"),
            kind => kind.contains("comment"),
        };
    if is_comment || node.child_count() == 0 {
        let kind = if is_comment {
            LineKind::Comment
        } else {
            LineKind::Code
        };
        // Tokens such as template text span lines they contribute only whitespace to.
        let segments = source[node.byte_range()].split('\n');
        for (row, segment) in (node.start_position().row..).zip(segments) {
            if !segment.trim().is_empty() && kinds[row] != LineKind::Code {
                kinds[row] = kind;
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        mark_lines(source, child, kinds);
    }
}

#[derive(Clone, Copy, Default)]
struct Nesting {
    /// Nesting level for cognitive complexity, which nested functions also increase.
    cognitive: u32,
    /// Depth of control structures.
    control: u32,
    function: u32,
}

/// Counts cyclomatic complexity (McCabe: one per function and per decision) and cognitive
/// complexity (a simplified SonarSource model: one per control structure plus its nesting level,
/// one per `else` or chained branch, and one per sequence of like logical operators).
struct ComplexityCounter<'a> {
    profile: &'a Profile,
    function_count: u32,
    cyclomatic: u32,
    cognitive: u32,
    max_nesting_depth: u32,
}

impl ComplexityCounter<'_> {
    fn visit(&mut self, node: Node, nesting: Nesting) {
        let kind = node.kind();
        let profile = self.profile;
        let mut inner = nesting;
        if !node.is_named() {
            if profile.logical_operators.contains(&kind) {
                self.cyclomatic += 1;
                if !continues_logical_sequence(node) {
                    self.cognitive += 1;
                }
            } else if kind == "else" && is_else_branch(node, profile) {
                self.cognitive += 1;
            }
        } else if profile.functions.contains(&kind) && !is_function_type(node) {
            self.function_count += 1;
            self.cyclomatic += 1;
            if nesting.function > 0 {
                inner.cognitive += 1;
            }
            inner.function += 1;
        } else if profile.branches.contains(&kind) {
            self.cyclomatic += 1;
            // `else if` is scored by its `else` and continues the chain at the same level.
            if !follows_else(node) {
                self.cognitive += 1 + nesting.cognitive;
                inner.cognitive += 1;
                inner.control += 1;
            }
        } else if profile.chained_branches.contains(&kind) {
            self.cyclomatic += 1;
            self.cognitive += 1;
        } else if profile.switches.contains(&kind) {
            self.cognitive += 1 + nesting.cognitive;
            inner.cognitive += 1;
            inner.control += 1;
        } else if profile.cases.contains(&kind) && !is_default_case(node) {
            self.cyclomatic += 1;
        }
        self.max_nesting_depth = self.max_nesting_depth.max(inner.control);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, inner);
        }
    }
}

/// Whether the operator's left operand is a binary expression with the same operator, as in
/// `a && b && c`.
fn continues_logical_sequence(operator: Node) -> bool {
    let Some(left) = operator.prev_sibling() else {
        return false;
    };
    let mut cursor = left.walk();
    left.children(&mut cursor)
        .any(|child| !child.is_named() && child.kind() == operator.kind())
}

/// Haskell's grammar names function types `function` as well as function declarations.
fn is_function_type(node: Node) -> bool {
    node.child_by_field_name("result").is_some()
}

fn follows_else(node: Node) -> bool {
    node.prev_sibling()
        .is_some_and(|sibling| !sibling.is_named() && sibling.kind() == "else")
}

/// Excludes `else` tokens that do not open a branch of their own: the fallback arm of a switch
/// and the second operand of a conditional expression such as Python's `a if c else b`.
fn is_else_branch(token: Node, profile: &Profile) -> bool {
    token.parent().is_none_or(|parent| {
        !profile.cases.contains(&parent.kind()) && parent.kind() != "conditional_expression"
    })
}

fn is_default_case(node: Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && matches!(child.kind(), "default" | "else" | "_"))
}
