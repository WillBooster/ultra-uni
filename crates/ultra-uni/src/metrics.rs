use serde::Serialize;
use tree_sitter::{Node, Tree};

use crate::language::Profile;
use crate::tree::{first_code_child, next_code_sibling, prev_code_sibling, walk};

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
    Unmarked,
    Comment,
    Code,
}

/// A line is blank when it holds only whitespace, a comment line when every token on it is part of
/// a comment, and a code line otherwise.
fn measure_lines(source: &str, tree: Option<&Tree>) -> LineMetrics {
    let Some(tree) = tree else {
        let mut total = 0;
        let mut blank = 0;
        for line in source.lines() {
            total += 1;
            if line.trim().is_empty() {
                blank += 1;
            }
        }
        return LineMetrics {
            total,
            code: total - blank,
            comment: 0,
            blank,
        };
    };
    let mut kinds: Vec<LineKind> = source
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                LineKind::Blank
            } else {
                LineKind::Unmarked
            }
        })
        .collect();
    mark_lines(source, tree.root_node(), &mut kinds);
    let mut metrics = LineMetrics {
        total: kinds.len() as u32,
        code: 0,
        comment: 0,
        blank: 0,
    };
    for kind in kinds {
        match kind {
            LineKind::Blank => metrics.blank += 1,
            LineKind::Comment => metrics.comment += 1,
            LineKind::Code | LineKind::Unmarked => metrics.code += 1,
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
    fn visit(&mut self, node: Node, nesting: Nesting) -> Nesting {
        let mut inner = nesting;
        if let Some(operator) = self.logical_operator(node) {
            self.cyclomatic += 1;
            if !self.continues_logical_sequence(node, operator) {
                self.cognitive += 1;
            }
        } else if !node.is_named() {
            self.visit_token(node);
        } else {
            self.visit_named(node, &mut inner);
        }
        self.max_nesting_depth = self.max_nesting_depth.max(inner.control);
        inner
    }

    fn visit_token(&mut self, token: Node) {
        if token.kind() == "else" && is_else_branch(token, self.profile) {
            self.cognitive += 1;
        } else if is_pattern_guard_keyword(token) {
            self.cyclomatic += 1;
            self.cognitive += 1;
        }
    }

    fn visit_named(&mut self, node: Node, inner: &mut Nesting) {
        let kind = node.kind();
        let profile = self.profile;
        if is_function_definition(node, profile) {
            self.visit_function(node, inner);
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
            if !follows_else(node, profile) {
                self.enter_control(inner);
            }
        } else if adds_else_scored_path(node, profile) {
            self.cyclomatic += 1;
        } else if profile.switches.contains(&kind) {
            self.enter_control(inner);
        } else if profile.cases.contains(&kind) {
            self.score_case(node);
        }
    }

    fn visit_function(&mut self, node: Node, inner: &mut Nesting) {
        if !continues_definition(node, self.source) {
            self.function_count += 1;
            self.cyclomatic += 1;
        }
        if inner.function > 0 {
            inner.cognitive += 1;
        }
        inner.function += 1;
        // Haskell's `\case` is a function whose body is a switch.
        if self.profile.switches.contains(&node.kind()) {
            self.enter_control(inner);
        }
    }

    fn enter_control(&mut self, inner: &mut Nesting) {
        self.cognitive += 1 + u64::from(inner.cognitive);
        inner.cognitive += 1;
        inner.control += 1;
    }

    fn score_case(&mut self, node: Node) {
        if !is_default_case(node, self.source, self.profile) {
            self.cyclomatic += 1;
            // Haskell's `guards` clause is itself a guard.
            let guard = node.child_by_field_name("guard");
            if guard.is_some() && guard == first_code_child(node) {
                self.cognitive += 1;
            }
        }
        if has_condition_guard(node) {
            self.cyclomatic += 1;
            self.cognitive += 1;
        }
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
        // A token between patterns, or directly under a pattern node or a control structure, joins
        // patterns, as Dart's `case 1 || 2`, `case [1 || 2]`, and `var (int y || int y) = x` do.
        let is_pattern =
            |node: Option<Node>| node.is_some_and(|node| node.kind().ends_with("pattern"));
        let in_pattern = is_pattern(prev_code_sibling(node))
            || is_pattern(next_code_sibling(node))
            || node.parent().is_some_and(|parent| {
                let kind = parent.kind();
                let profile = self.profile;
                kind.ends_with("pattern")
                    || profile.cases.contains(&kind)
                    || profile.branches.contains(&kind)
                    || profile.switches.contains(&kind)
            });
        (is_binary_operator
            || !in_pattern
                && is_operand(prev_code_sibling(node))
                && is_operand(next_code_sibling(node)))
        .then_some(*operator)
    }

    /// Whether the operator continues a sequence of the same operator, as in `a && b && c`: an
    /// adjacent operand contains it (nested chains) or the nearest earlier logical operator in the
    /// same chain is it (flat chains such as Dart's `??` and Haskell's `operator_chain`).
    fn continues_logical_sequence(&self, node: Node, operator: &str) -> bool {
        let in_operand = [prev_code_sibling(node), next_code_sibling(node)]
            .into_iter()
            .flatten()
            .any(|operand| {
                let mut cursor = operand.walk();
                operand
                    .children(&mut cursor)
                    .any(|child| self.logical_operator(child) == Some(operator))
            });
        let earlier_in_chain = std::iter::successors(node.prev_sibling(), Node::prev_sibling)
            .find_map(|sibling| self.logical_operator(sibling))
            == Some(operator);
        in_operand || earlier_in_chain
    }
}

fn is_function_definition(node: Node, profile: &Profile) -> bool {
    let kind = node.kind();
    profile.functions.contains(&kind)
        && !is_function_type(node)
        && (!profile.optional_body_functions.contains(&kind) || has_body(node))
}

/// Constructs whose `else` token already scores their cognitive point: Rust's `else if`
/// (`else_clause` with a `condition`) and `alternative_branches` such as Rust's `let ... else`.
fn adds_else_scored_path(node: Node, profile: &Profile) -> bool {
    let kind = node.kind();
    (kind == "else_clause" && node.child_by_field_name("condition").is_some())
        || (profile.alternative_branches.contains(&kind)
            && node.child_by_field_name("alternative").is_some())
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

// Dart's `when` and Kotlin's guarded `when_entry` use bare keywords instead of a branch node.
fn is_pattern_guard_keyword(token: Node) -> bool {
    !token.is_named()
        && ((token.kind() == "when" && token.prev_sibling().is_some())
            || (token.kind() == "if"
                && token.parent().is_some_and(|parent| {
                    parent.kind() == "when_entry" && parent.child_by_field_name("guard").is_some()
                })))
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
            .any(|child| matches!(child.kind(), "function_body" | "block"))
}

/// Whether the node is a further equation of the Haskell definition before it, since Haskell
/// writes one function as several equations with the same name; other languages' same-name
/// siblings are overloads, each its own function.
fn continues_definition(node: Node, source: &str) -> bool {
    if node.kind() != "function" {
        return false;
    }
    let name = |node: Node| {
        node.child_by_field_name("name")
            .map(|name| &source[name.byte_range()])
    };
    std::iter::successors(node.prev_named_sibling(), Node::prev_named_sibling)
        .find(|sibling| !sibling.is_extra())
        .is_some_and(|previous| {
            previous.kind() == "function"
                && name(previous).is_some()
                && name(previous) == name(node)
        })
}

/// Haskell's grammar names function types `function` as well as function declarations.
fn is_function_type(node: Node) -> bool {
    node.child_by_field_name("result").is_some()
}

/// Whether the branch is the `if` of an `else if`: it follows an `else` token and has the kind of
/// the conditional owning that `else`, so a loop used as an unbraced `else` body is not one.
fn follows_else(node: Node, profile: &Profile) -> bool {
    let Some(token) =
        prev_code_sibling(node).filter(|sibling| !sibling.is_named() && sibling.kind() == "else")
    else {
        return false;
    };
    is_else_branch(token, profile)
        && else_owner(token).is_some_and(|owner| owner.kind() == node.kind())
}

/// Excludes `else` tokens that do not open a branch of their own: the fallback arm of a switch
/// and the second operand of a conditional expression such as Python's `a if c else b`.
fn is_else_branch(token: Node, profile: &Profile) -> bool {
    else_owner(token).is_none_or(|owner| {
        let kind = owner.kind();
        !profile.cases.contains(&kind)
            && !profile.switches.contains(&kind)
            && kind != "conditional_expression"
    })
}

/// Whether the pattern's only code token, besides grouping parentheses and extras such as
/// comments, is the wildcard `_` or, in a Haskell guard, `otherwise`, so `(_)` matches anything
/// like `_`; elsewhere `otherwise` is an ordinary name.
fn is_wildcard_pattern(pattern: Node, source: &str, is_guard_clause: bool) -> bool {
    let mut sole_token: Option<&str> = None;
    let mut too_many = false;
    walk(pattern, |node, _| {
        if too_many || node.is_extra() {
            return false;
        }
        if node.child_count() == 0 && !matches!(node.kind(), "(" | ")") {
            if sole_token.is_some() {
                too_many = true;
                return false;
            }
            sole_token = Some(&source[node.byte_range()]);
        }
        true
    });
    !too_many
        && match sole_token {
            Some("_") => true,
            Some("otherwise") => is_guard_clause,
            _ => false,
        }
}

/// The construct an `else` token belongs to, looking through the node that wraps the keyword and
/// its body: `else_clause` in C-like grammars and `else` in Ruby.
fn else_owner(token: Node) -> Option<Node> {
    let parent = token.parent()?;
    if matches!(parent.kind(), "else_clause" | "else") {
        parent.parent()
    } else {
        Some(parent)
    }
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
        .or_else(|| first_code_child(node));
    let is_catch_all = profile.wildcard_cases.contains(&node.kind())
        && pattern.is_some_and(|pattern| {
            // A comma after the pattern makes the label a sequence, as in Python's `case _,:`.
            is_wildcard_pattern(pattern, source, node.kind() == "guards")
                && next_code_sibling(pattern).is_none_or(|next| next.kind() != ",")
        });
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
            .any(|child| is_pattern_guard_keyword(child))
        // A Haskell alternative keeps its guards in its `match`.
        || node
            .child_by_field_name("match")
            .is_some_and(|m| m.child_by_field_name("guards").is_some());
    (has_default_keyword || is_catch_all) && !has_guard
}
