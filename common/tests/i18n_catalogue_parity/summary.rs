//! Summarize parsed Fluent resources for the catalogue parity gates.
//!
//! The gates compare catalogues structurally, so each pattern is reduced to
//! the facts they inspect: the variables it references, its literal text, and
//! the keys of its select expressions. Walking the `fluent-syntax` tree keeps
//! those facts exact where a regular expression over raw text would miscount
//! variables that appear only inside a select variant.

use std::collections::{BTreeMap, BTreeSet};

use fluent_syntax::{
    ast::{
        CallArguments,
        Entry,
        Expression,
        InlineExpression,
        Pattern,
        PatternElement,
        VariantKey,
    },
    parser,
};

/// Record one select expression: its selector variable and its variant keys.
#[derive(Debug, Default)]
pub struct Select {
    /// Name of the selector when it is a plain variable reference.
    pub selector: Option<String>,
    /// Variant keys in source order; number literals keep their digits.
    pub keys: Vec<String>,
}

/// Record the facts the parity gates need about one message pattern.
#[derive(Debug, Default)]
pub struct PatternSummary {
    /// Every variable the pattern references, in any variant.
    pub variables: BTreeSet<String>,
    /// The literal text of the pattern, across all variants.
    pub text: String,
    /// Every select expression in the pattern, outermost first.
    pub selects: Vec<Select>,
}

/// Map `id`, `id.attribute`, and `-term` keys to their pattern summaries.
pub type Catalogue = BTreeMap<String, PatternSummary>;

/// Parse `source` and summarize each message, attribute, and term.
///
/// # Errors
///
/// Returns the parser's findings when `source` is not valid Fluent syntax.
/// Fluent recovers from a malformed entry by emitting junk, which the loader
/// would silently drop, so any parser error rejects the whole resource.
pub fn summarize(source: &str) -> Result<Catalogue, String> {
    let resource = parser::parse(source).map_err(|(_, errors)| format!("{errors:?}"))?;
    let mut catalogue = Catalogue::new();
    for entry in &resource.body {
        match entry {
            Entry::Message(message) => {
                let id = message.id.name;
                if let Some(value) = &message.value {
                    catalogue.insert(id.to_owned(), summarize_pattern(value));
                }
                for attribute in &message.attributes {
                    let key = format!("{id}.{}", attribute.id.name);
                    catalogue.insert(key, summarize_pattern(&attribute.value));
                }
            }
            Entry::Term(term) => {
                let key = format!("-{}", term.id.name);
                catalogue.insert(key, summarize_pattern(&term.value));
            }
            _ => {}
        }
    }
    Ok(catalogue)
}

/// Collect the variables, text, and selects of one pattern.
fn summarize_pattern(pattern: &Pattern<&str>) -> PatternSummary {
    let mut summary = PatternSummary::default();
    visit_pattern(pattern, &mut summary);
    summary
}

/// Visit every element of `pattern`, including nested variant patterns.
fn visit_pattern(pattern: &Pattern<&str>, summary: &mut PatternSummary) {
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => summary.text.push_str(value),
            PatternElement::Placeable { expression } => visit_expression(expression, summary),
        }
    }
}

/// Visit a placeable expression, recording a select and its variants.
fn visit_expression(expression: &Expression<&str>, summary: &mut PatternSummary) {
    match expression {
        Expression::Inline(inline) => visit_inline(inline, summary),
        Expression::Select { selector, variants } => {
            visit_inline(selector, summary);
            let keys = variants
                .iter()
                .map(|variant| match &variant.key {
                    VariantKey::Identifier { name } => (*name).to_owned(),
                    VariantKey::NumberLiteral { value } => (*value).to_owned(),
                })
                .collect();
            summary.selects.push(Select {
                selector: variable_name(selector),
                keys,
            });
            for variant in variants {
                visit_pattern(&variant.value, summary);
            }
        }
    }
}

/// Record a variable reference, descending into nested placeables.
fn visit_inline(inline: &InlineExpression<&str>, summary: &mut PatternSummary) {
    match inline {
        InlineExpression::VariableReference { id } => {
            summary.variables.insert(id.name.to_owned());
        }
        InlineExpression::Placeable { expression } => visit_expression(expression, summary),
        InlineExpression::FunctionReference { arguments, .. } => {
            visit_arguments(arguments, summary);
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => visit_arguments(arguments, summary),
        _ => {}
    }
}

/// Record the variables passed to a function or parameterized term.
fn visit_arguments(arguments: &CallArguments<&str>, summary: &mut PatternSummary) {
    for positional in &arguments.positional {
        visit_inline(positional, summary);
    }
    for named in &arguments.named {
        visit_inline(&named.value, summary);
    }
}

/// Return the variable a selector names, if it is a plain variable reference.
fn variable_name(inline: &InlineExpression<&str>) -> Option<String> {
    match inline {
        InlineExpression::VariableReference { id } => Some(id.name.to_owned()),
        _ => None,
    }
}
