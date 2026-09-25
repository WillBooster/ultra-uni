use tree_sitter::Language;

/// Identifiers follow Exercode's judge language IDs, plus `tsx`.
pub const LANGUAGE_IDS: &[&str] = &[
    "c",
    "cpp",
    "csharp",
    "css",
    "dart",
    "haskell",
    "html",
    "java",
    "javascript",
    "jsp",
    "kotlin",
    "php",
    "python",
    "ruby",
    "rust",
    "text",
    "tsx",
    "typescript",
    "zig",
];

/// Node kinds that drive complexity metrics; each list names kinds of the language's grammar.
pub struct Profile {
    pub functions: &'static [&'static str],
    /// Conditionals, loops, and handlers: they add a path and nest the code inside them.
    pub branches: &'static [&'static str],
    /// Branches that continue a chain (e.g., `elif`) and therefore do not nest.
    pub chained_branches: &'static [&'static str],
    /// Multi-way branches, whose paths are counted by their `cases` instead.
    pub switches: &'static [&'static str],
    pub cases: &'static [&'static str],
    pub logical_operators: &'static [&'static str],
}

pub struct LanguageSpec {
    pub grammar: Option<Language>,
    /// `None` for languages without functions or control flow, such as markup and style sheets.
    pub profile: Option<&'static Profile>,
}

pub fn resolve(id: &str) -> Option<LanguageSpec> {
    let (grammar, profile): (Option<Language>, Option<&'static Profile>) = match id {
        "c" => (Some(tree_sitter_c::LANGUAGE.into()), Some(&C)),
        "cpp" => (Some(tree_sitter_cpp::LANGUAGE.into()), Some(&CPP)),
        "csharp" => (Some(tree_sitter_c_sharp::LANGUAGE.into()), Some(&CSHARP)),
        "css" => (Some(tree_sitter_css::LANGUAGE.into()), None),
        "dart" => (Some(tree_sitter_dart::LANGUAGE.into()), Some(&DART)),
        "haskell" => (Some(tree_sitter_haskell::LANGUAGE.into()), Some(&HASKELL)),
        "html" => (Some(tree_sitter_html::LANGUAGE.into()), None),
        "java" => (Some(tree_sitter_java::LANGUAGE.into()), Some(&JAVA)),
        "javascript" => (
            Some(tree_sitter_javascript::LANGUAGE.into()),
            Some(&JAVASCRIPT),
        ),
        // JSP scriptlets share ERB's `<% %>` delimiters, so the embedded-template grammar splits
        // them from the surrounding markup.
        "jsp" => (Some(tree_sitter_embedded_template::LANGUAGE.into()), None),
        "kotlin" => (Some(tree_sitter_kotlin_ng::LANGUAGE.into()), Some(&KOTLIN)),
        "php" => (Some(tree_sitter_php::LANGUAGE_PHP.into()), Some(&PHP)),
        "python" => (Some(tree_sitter_python::LANGUAGE.into()), Some(&PYTHON)),
        "ruby" => (Some(tree_sitter_ruby::LANGUAGE.into()), Some(&RUBY)),
        "rust" => (Some(tree_sitter_rust::LANGUAGE.into()), Some(&RUST)),
        "text" => (None, None),
        "tsx" => (
            Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Some(&JAVASCRIPT),
        ),
        "typescript" => (
            Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Some(&JAVASCRIPT),
        ),
        "zig" => (Some(tree_sitter_zig::LANGUAGE.into()), Some(&ZIG)),
        _ => return None,
    };
    Some(LanguageSpec { grammar, profile })
}

const C_LIKE_LOGICAL_OPERATORS: &[&str] = &["&&", "||"];

const C: Profile = Profile {
    functions: &["function_definition"],
    branches: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "do_statement",
        "conditional_expression",
    ],
    chained_branches: &[],
    switches: &["switch_statement"],
    cases: &["case_statement"],
    logical_operators: C_LIKE_LOGICAL_OPERATORS,
};

const CPP: Profile = Profile {
    functions: &["function_definition", "lambda_expression"],
    branches: &[
        "if_statement",
        "for_statement",
        "for_range_loop",
        "while_statement",
        "do_statement",
        "conditional_expression",
        "catch_clause",
    ],
    chained_branches: &[],
    switches: &["switch_statement"],
    cases: &["case_statement"],
    logical_operators: &["&&", "||", "and", "or"],
};

const CSHARP: Profile = Profile {
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "local_function_statement",
        "lambda_expression",
        "anonymous_method_expression",
        "operator_declaration",
        "conversion_operator_declaration",
        "accessor_declaration",
    ],
    branches: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "conditional_expression",
        "catch_clause",
    ],
    chained_branches: &[],
    switches: &["switch_statement", "switch_expression"],
    cases: &["switch_section", "switch_expression_arm"],
    logical_operators: &["&&", "||", "??"],
};

const DART: Profile = Profile {
    functions: &["function_body", "function_expression"],
    branches: &[
        "if_statement",
        "if_element",
        "for_statement",
        "for_element",
        "while_statement",
        "do_statement",
        "conditional_expression",
        "catch_clause",
    ],
    chained_branches: &[],
    switches: &["switch_statement", "switch_expression"],
    cases: &["switch_statement_case", "switch_expression_case"],
    logical_operators: &["&&", "||", "??"],
};

const HASKELL: Profile = Profile {
    functions: &["function", "lambda", "lambda_case"],
    branches: &["conditional"],
    chained_branches: &[],
    switches: &["case", "multi_way_if"],
    cases: &["alternative", "guards"],
    logical_operators: &[],
};

const JAVA: Profile = Profile {
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
        "lambda_expression",
    ],
    branches: &[
        "if_statement",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "ternary_expression",
        "catch_clause",
    ],
    chained_branches: &[],
    switches: &["switch_expression"],
    cases: &["switch_label"],
    logical_operators: C_LIKE_LOGICAL_OPERATORS,
};

/// Shared by JavaScript, TypeScript, and TSX, whose grammars use the same node kinds.
const JAVASCRIPT: Profile = Profile {
    functions: &[
        "function_declaration",
        "function_expression",
        "generator_function",
        "generator_function_declaration",
        "arrow_function",
        "method_definition",
    ],
    branches: &[
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "ternary_expression",
        "catch_clause",
    ],
    chained_branches: &[],
    switches: &["switch_statement"],
    cases: &["switch_case"],
    logical_operators: &["&&", "||", "??"],
};

const KOTLIN: Profile = Profile {
    functions: &[
        "function_declaration",
        "anonymous_function",
        "lambda_literal",
        "secondary_constructor",
        "getter",
        "setter",
    ],
    branches: &[
        "if_expression",
        "for_statement",
        "while_statement",
        "do_while_statement",
        "catch_block",
    ],
    chained_branches: &[],
    switches: &["when_expression"],
    cases: &["when_entry"],
    logical_operators: C_LIKE_LOGICAL_OPERATORS,
};

const PHP: Profile = Profile {
    functions: &[
        "function_definition",
        "method_declaration",
        "anonymous_function",
        "arrow_function",
    ],
    branches: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "conditional_expression",
        "catch_clause",
    ],
    chained_branches: &["else_if_clause"],
    switches: &["switch_statement", "match_expression"],
    cases: &["case_statement", "match_conditional_expression"],
    logical_operators: &["&&", "||", "and", "or", "??"],
};

const PYTHON: Profile = Profile {
    functions: &["function_definition", "lambda"],
    branches: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "conditional_expression",
        "except_clause",
    ],
    chained_branches: &["elif_clause"],
    switches: &["match_statement"],
    cases: &["case_clause"],
    logical_operators: &["and", "or"],
};

const RUBY: Profile = Profile {
    functions: &["method", "singleton_method", "lambda", "block", "do_block"],
    branches: &[
        "if",
        "unless",
        "while",
        "until",
        "for",
        "conditional",
        "rescue",
        "if_modifier",
        "unless_modifier",
        "while_modifier",
        "until_modifier",
        "rescue_modifier",
    ],
    chained_branches: &["elsif"],
    switches: &["case", "case_match"],
    cases: &["when", "in_clause"],
    logical_operators: &["&&", "||", "and", "or"],
};

const RUST: Profile = Profile {
    functions: &["function_item", "closure_expression"],
    branches: &[
        "if_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ],
    chained_branches: &[],
    switches: &["match_expression"],
    cases: &["match_arm"],
    logical_operators: C_LIKE_LOGICAL_OPERATORS,
};

const ZIG: Profile = Profile {
    functions: &["function_declaration"],
    branches: &[
        "if_statement",
        "if_expression",
        "for_statement",
        "for_expression",
        "while_statement",
        "while_expression",
        "catch_expression",
    ],
    chained_branches: &[],
    switches: &["switch_expression"],
    cases: &["switch_case"],
    logical_operators: &["and", "or"],
};

#[cfg(test)]
mod tests {
    use super::*;

    // A misspelled kind never matches any node, which silently lowers the metrics.
    #[test]
    fn profiles_name_existing_node_kinds() {
        for id in LANGUAGE_IDS {
            let spec = resolve(id).unwrap();
            let (Some(grammar), Some(profile)) = (spec.grammar, spec.profile) else {
                continue;
            };
            let named_kinds = [
                profile.functions,
                profile.branches,
                profile.chained_branches,
                profile.switches,
                profile.cases,
            ];
            for kind in named_kinds.concat() {
                assert_ne!(grammar.id_for_node_kind(kind, true), 0, "{id}: {kind}");
            }
            for kind in profile.logical_operators {
                assert_ne!(grammar.id_for_node_kind(kind, false), 0, "{id}: {kind}");
            }
        }
    }
}
