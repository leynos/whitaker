//! Driver-private unit tests for the two branch pairs a hand-built value can
//! reach: the diagnostic-span choice and the interim path gate.
//!
//! Both failures would be silent. A diagnostic attached to a dummy span is
//! dropped from the report rather than misreported, and a path gate that
//! selected every source would diagnose virtual files that have no source to
//! point at. Every other driver mapping reads a shape only a real compilation
//! produces, so the UI fixtures cover those instead; a unit test that
//! assembles an AST node by hand would prove the match arm and not that rustc
//! hands it that shape.

use camino::Utf8PathBuf;
use rustc_span::{BytePos, DUMMY_SP, Span, SyntaxContext};

use super::{
    diagnostics::diagnostic_span,
    facts::{PathPolicy, SourceFile},
};
use crate::policy::PathSelection;

/// A non-dummy span, used where the distinction from [`DUMMY_SP`] matters.
fn real_span() -> Span { Span::new(BytePos(10), BytePos(20), SyntaxContext::root(), None) }

/// The item-span fallback fires only for a dummy identifier span.
#[test]
fn diagnostic_span_prefers_the_identifier_unless_it_is_dummy() {
    let item_span = real_span();
    assert_eq!(
        diagnostic_span(DUMMY_SP, item_span),
        item_span,
        "a dummy identifier span must fall back to the item span"
    );
    assert_eq!(
        diagnostic_span(item_span, DUMMY_SP),
        item_span,
        "a real identifier span must be used even when the item span is dummy"
    );
}

/// The interim path gate selects real files and rejects virtual ones.
///
/// This is the D-2 interim behaviour: until include and exclude patterns
/// arrive, every real source file is selected. The three virtual kinds stand
/// for a macro expansion, a `--cfg` specification, and an anonymous file.
#[test]
fn interim_policy_selects_real_files_and_rejects_virtual_ones() {
    let real = SourceFile::Real(Utf8PathBuf::from("src/lib.rs"));
    assert_eq!(
        PathPolicy::InterimUnrestricted.select(&real),
        PathSelection::Selected,
        "a real source file must be selected under the interim policy"
    );
    assert_eq!(
        PathPolicy::InterimUnrestricted.select(&SourceFile::Virtual),
        PathSelection::NotSelected,
        "a virtual source must never be selected"
    );
}
