//! Lightweight expression helpers for lint analysis.

use crate::path::SimplePath;

/// A tiny expression model used for helper functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A call expression with a resolved callee path.
    Call { callee: SimplePath },
    /// A path expression.
    Path(SimplePath),
    /// Any other literal expression (placeholder for expansion).
    Literal(String),
}

/// Returns the callee path of a call expression, if one is present.
///
/// # Examples
///
/// ```
/// use common::expr::{Expr, def_id_of_expr_callee};
/// use common::path::SimplePath;
///
/// let expr = Expr::Call { callee: SimplePath::from("std::mem::drop") };
/// assert_eq!(
///     def_id_of_expr_callee(&expr).unwrap().segments(),
///     &["std", "mem", "drop"]
/// );
/// ```
#[must_use]
pub fn def_id_of_expr_callee(expr: &Expr) -> Option<&SimplePath> {
    match expr {
        Expr::Call { callee } => Some(callee),
        _ => None,
    }
}

/// Tests whether a path matches the provided candidate segments.
///
/// # Examples
///
/// ```
/// use common::expr::is_path_to;
/// use common::path::SimplePath;
///
/// let path = SimplePath::from("core::option::Option");
/// assert!(is_path_to(&path, ["core", "option", "Option"]));
/// ```
#[must_use]
pub fn is_path_to<'a, I>(path: &SimplePath, candidate: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    path.matches(candidate)
}

/// Returns `true` when the receiver is `Option` or `Result` regardless of module
/// path.
///
/// # Examples
///
/// ```
/// use common::expr::recv_is_option_or_result;
/// use common::path::SimplePath;
///
/// assert!(recv_is_option_or_result(&SimplePath::from("std::option::Option")));
/// assert!(recv_is_option_or_result(&SimplePath::from("Result")));
/// assert!(!recv_is_option_or_result(&SimplePath::from("crate::Thing")));
/// ```
#[must_use]
pub fn recv_is_option_or_result(path: &SimplePath) -> bool {
    matches!(path.last(), Some("Option" | "Result"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn callee_extraction() {
        let expr = Expr::Call {
            callee: SimplePath::from("std::mem::drop"),
        };
        assert!(def_id_of_expr_callee(&expr).is_some());
    }

    #[rstest]
    fn recognises_option_like_receivers() {
        let option_path = SimplePath::from("std::option::Option");
        let result_path = SimplePath::from("Result");
        let custom_path = SimplePath::from("crate::Thing");

        assert!(recv_is_option_or_result(&option_path));
        assert!(recv_is_option_or_result(&result_path));
        assert!(!recv_is_option_or_result(&custom_path));
    }
}
