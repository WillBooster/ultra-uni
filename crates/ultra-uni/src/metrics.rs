use serde::Serialize;
use tree_sitter::{Node, Tree};

use crate::language::Profile;
use crate::tree::walk;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub lines: LineMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyclomatic_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Grows quadratically with nesting, so deeply nested input would overflow `u32`.
    pub cognitive_complexity: Option<u64>,
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
        source,
        profile,
        function_count: 0,
        // The file itself is one path even without any function.
        cyclomatic: 1,
        cognitive: 0,
        max_nesting_depth: 0,
    };
    // `nestings[depth]` holds the nesting that the node last visited at `depth` gives its children.
    let mut nestings: Vec<Nesting> = Vec::new();
    walk(tree.root_node(), |node, depth| {
        let outer = depth
            .checked_sub(1)
            .map_or_else(Nesting::default, |parent| nestings[parent]);
        nestings.truncate(depth);
        nestings.push(counter.visit(node, outer));
        true
    });
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
            LineKind::Comment => metrics.comment += 1,
            LineKind::Code | LineKind::Blank => metrics.code += 1,
        }
    }
    metrics
}

fn mark_lines(source: &str, root: Node, kinds: &mut [LineKind]) {
    walk(root, |node, _| {
        let is_comment = node.is_named()
            && match node.kind() {
                "haddock" => true,
                // The embedded-template grammar used for JSP parses `<%-- --%>` as a directive.
                "directive" | "output_directive" => source[node.start_byte()..].starts_with("<%--"),
                kind => kind.contains("comment"),
            };
        if !is_comment && node.child_count() > 0 {
            return true;
        }
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
        false
    });
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
    source: &'a str,
    profile: &'a Profile,
    function_count: u32,
    cyclomatic: u32,
    cognitive: u64,
    max_nesting_depth: u32,
}

impl ComplexityCounter<'_> {
    /// Scores `node` and returns the nesting its children are in.
    fn visit(&mut self, node: Node, nesting: Nesting) -> Nesting {
        let kind = node.kind();
        let profile = self.profile;
        let mut inner = nesting;
        if let Some(operator) = self.logical_operator(node) {
            self.cyclomatic += 1;
            if !self.continues_logical_sequence(node, operator) {
                self.cognitive += 1;
            }
        } else if !node.is_named() {
            if kind == "else" && is_else_branch(node, profile) {
                self.cognitive += 1;
            } else if is_when_guard(node) {
                self.cyclomatic += 1;
                self.cognitive += 1;
            }
        } else if profile.functions.contains(&kind)
            && !is_function_type(node)
            && !(profile.optional_body_functions.contains(&kind) && !has_body(node))
        {
            self.function_count += 1;
            self.cyclomatic += 1;
            if nesting.function > 0 {
                inner.cognitive += 1;
            }
            inner.function += 1;
        } else if profile.chained_branches.contains(&kind)
            || (profile.branches.contains(&kind) && is_guard(node))
        {
            self.cyclomatic += 1;
            self.cognitive += 1;
        } else if profile.branches.contains(&kind)
            || is_comprehension_filter(node)
            || is_dart_handler_block(node)
        {
            self.cyclomatic += 1;
            // `else if` is scored by its `else` and continues the chain at the same level.
            if !follows_else(node) {
                self.cognitive += 1 + u64::from(nesting.cognitive);
                inner.cognitive += 1;
                inner.control += 1;
            }
        } else if profile.alternative_branches.contains(&kind) {
            if node.child_by_field_name("alternative").is_some() {
                self.cyclomatic += 1;
            }
        } else if profile.switches.contains(&kind) {
            self.cognitive += 1 + u64::from(nesting.cognitive);
            inner.cognitive += 1;
            inner.control += 1;
        } else if profile.cases.contains(&kind) {
            if !is_default_case(node, self.source, profile) {
                self.cyclomatic += 1;
                // Haskell's `guards` clause is itself a guard.
                let guard = node.child_by_field_name("guard");
                if guard.is_some() && guard == node.named_child(0) {
                    self.cognitive += 1;
                }
            }
            if has_condition_guard(node) {
                self.cyclomatic += 1;
                self.cognitive += 1;
            }
        }
        self.max_nesting_depth = self.max_nesting_depth.max(inner.control);
        inner
    }
}

impl<'a> ComplexityCounter<'a> {
    /// Returns the operator of a binary logical expression, excluding the same token used
    /// otherwise, such as C++'s rvalue reference `T&&` and Haskell's operator section `(&&)`.
    fn logical_operator(&self, node: Node) -> Option<&'a str> {
        let text = if !node.is_named() {
            node.kind()
        } else if Some(node.kind()) == self.profile.operator_node {
            &self.source[node.byte_range()]
        } else {
            return None;
        };
        let operator = self
            .profile
            .logical_operators
            .iter()
            .find(|&&op| op == text)?;
        // An operand can be an anonymous keyword such as Zig's `unreachable`, so the parent's
        // `operator` field decides where the grammar provides one.
        let is_binary_operator = node.parent().is_some_and(|parent| {
            parent.child_by_field_name("operator") == Some(node)
                && parent.child_by_field_name("left").is_some()
                && parent.child_by_field_name("right").is_some()
        });
        let is_operand = |sibling: Option<Node>| sibling.is_some_and(|sibling| sibling.is_named());
        (is_binary_operator || is_operand(node.prev_sibling()) && is_operand(node.next_sibling()))
            .then_some(*operator)
    }

    /// Whether the operator continues a sequence of the same operator, as in `a && b && c`: an
    /// adjacent operand contains it (nested chains) or an earlier sibling is it (flat chains such as
    /// Dart's `??`).
    fn continues_logical_sequence(&self, node: Node, operator: &str) -> bool {
        let in_operand = [node.prev_sibling(), node.next_sibling()]
            .into_iter()
            .flatten()
            .any(|operand| {
                let mut cursor = operand.walk();
                operand
                    .children(&mut cursor)
                    .any(|child| self.logical_operator(child) == Some(operator))
            });
        let earlier_in_chain = std::iter::successors(node.prev_sibling(), Node::prev_sibling)
            .any(|sibling| self.logical_operator(sibling) == Some(operator));
        in_operand || earlier_in_chain
    }
}

/// Haskell's grammar names a list comprehension filter `boolean`, as it does a guard's condition,
/// which its `guards` case already scores.
fn is_comprehension_filter(node: Node) -> bool {
    node.kind() == "boolean"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "qualifiers")
}

/// Whether a Rust match arm's pattern carries an `if` guard in its `condition` field.
fn has_condition_guard(case: Node) -> bool {
    case.child_by_field_name("pattern")
        .is_some_and(|pattern| pattern.child_by_field_name("condition").is_some())
}

/// Whether the node is the body of a Dart exception handler; the grammar keeps a handler's `catch`
/// clause or `on` type as siblings of its block in the `try_statement`.
fn is_dart_handler_block(node: Node) -> bool {
    node.kind() == "block"
        && node.parent().is_some_and(|parent| {
            parent.kind() == "try_statement" && parent.child_by_field_name("body") != Some(node)
        })
}

/// Whether the token is the `when` of a Dart pattern guard, which the grammar leaves unwrapped in
/// the `if` or `case` holding the pattern. Other grammars' `when` keywords start their node.
fn is_when_guard(token: Node) -> bool {
    !token.is_named() && token.kind() == "when" && token.prev_sibling().is_some()
}

/// Whether the node is the guard of a case, as Python's `if_clause` is, which the grammar also uses
/// for comprehension filters.
fn is_guard(node: Node) -> bool {
    node.parent()
        .and_then(|parent| parent.child_by_field_name("guard"))
        .is_some_and(|guard| guard == node)
}

fn has_body(node: Node) -> bool {
    let mut cursor = node.walk();
    node.child_by_field_name("body").is_some()
        // C# properties and indexers keep an expression body in `value`, which also holds an
        // auto-property's initializer.
        || node
            .child_by_field_name("value")
            .is_some_and(|value| value.kind() == "arrow_expression_clause")
        || node
            .children(&mut cursor)
            .any(|child| child.kind() == "function_body")
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
    let mut owner = token.parent();
    // Ruby wraps the `else` keyword and its body in an `else` node.
    if let Some(parent) = owner.filter(|parent| parent.kind() == "else") {
        owner = parent.parent();
    }
    owner.is_none_or(|owner| {
        let kind = owner.kind();
        !profile.cases.contains(&kind)
            && !profile.switches.contains(&kind)
            && kind != "conditional_expression"
    })
}

/// A default case is marked by a `default` or `else` keyword, or has the wildcard `_` (or Haskell's
/// `otherwise` guard) as its entire pattern where the label is a pattern, and carries no guard.
fn is_default_case(node: Node, source: &str, profile: &Profile) -> bool {
    let mut cursor = node.walk();
    let has_default_keyword = node
        .children(&mut cursor)
        .any(|child| !child.is_named() && matches!(child.kind(), "default" | "else" | "_"));
    let pattern = node
        .child_by_field_name("pattern")
        .or_else(|| node.named_child(0));
    let is_catch_all = profile.wildcard_cases.contains(&node.kind())
        && pattern
            .is_some_and(|pattern| matches!(&source[pattern.byte_range()], "_" | "otherwise"));
    // Haskell's `guards` names its own condition `guard`; that condition is the pattern here.
    let has_guard = node
        .child_by_field_name("guard")
        .is_some_and(|guard| Some(guard) != pattern)
        || node
            .named_children(&mut cursor)
            .any(|child| profile.chained_branches.contains(&child.kind()))
        || has_condition_guard(node)
        || node
            .children(&mut cursor)
            .any(|child| is_when_guard(child))
        // A Haskell alternative keeps its guards in its `match`.
        || node
            .child_by_field_name("match")
            .is_some_and(|m| m.child_by_field_name("guards").is_some());
    (has_default_keyword || is_catch_all) && !has_guard
}
