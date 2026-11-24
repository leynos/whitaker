//! Reduces an AST item to the facts the selection predicate reads.
//!
//! This module holds every rustc-specific query the lint needs, so the policy
//! beside it stays compiler-free. It makes no decision of its own beyond
//! mapping: the harness comes from the session, provenance from the item span,
//! the path from the source map, and documentation from the attribute shapes.

use camino::Utf8PathBuf;
use rustc_ast::ast::{AttrVec, Fn as AstFn, Item};
use rustc_lint::{EarlyContext, LintContext};
use rustc_session::Session;
use rustc_span::FileName;

use super::attributes::doc_attribute_shape;
use crate::policy::{
    BodyPresence,
    CandidateFacts,
    FunctionContext,
    HarnessStatus,
    PathSelection,
    Provenance,
    documentation_status,
};

/// The source file an item was written in, as far as the source map knows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SourceFile {
    /// A real file on the local filesystem.
    Real(Utf8PathBuf),
    /// Any virtual source: a macro expansion, a `--cfg` specification, an
    /// anonymous file, or a doctest.
    Virtual,
}

/// Decides which source files the lint selects.
///
/// Until roadmap item 2.2.12 adds `dylint.toml` include and exclude patterns,
/// every real file is selected. The variant is named for its transience so the
/// debt is visible at every call site and the fixture
/// `fail_library_source_interim.rs` can record the behaviour it causes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PathPolicy {
    /// Selects every real source file, and no virtual one.
    InterimUnrestricted,
}

impl PathPolicy {
    /// Reports whether this policy selects `source`.
    pub(super) const fn select(self, source: &SourceFile) -> PathSelection {
        match (self, source) {
            (Self::InterimUnrestricted, SourceFile::Real(_)) => PathSelection::Selected,
            (Self::InterimUnrestricted, SourceFile::Virtual) => PathSelection::NotSelected,
        }
    }
}

/// The item-level inputs a candidate's facts are built from.
///
/// A struct rather than four more parameters: the facts builder already needs
/// the session, and five positional arguments is where transposition starts.
#[derive(Clone, Copy)]
struct ItemFacts<'a> {
    /// The item's outer and inner attributes.
    attrs: &'a AttrVec,
    /// The item's own span.
    span: rustc_span::Span,
    /// The function the item declares.
    function: &'a AstFn,
    /// Where the function sits in the item tree.
    context: FunctionContext,
}

/// Builds the facts for one function item.
///
/// Generic over the item's kind so the same builder serves a free function
/// (`Item<ItemKind>`) and an associated one (`Item<AssocItemKind>`); those two
/// types differ only in the kind they carry, which this builder never reads.
pub(super) fn item_facts<K>(
    cx: &EarlyContext<'_>,
    item: &Item<K>,
    function: &AstFn,
    context: FunctionContext,
) -> CandidateFacts {
    build(
        cx,
        ItemFacts {
            attrs: &item.attrs,
            span: item.span,
            function,
            context,
        },
    )
}

/// Assembles the six facts from the session and the item.
fn build(cx: &EarlyContext<'_>, item: ItemFacts<'_>) -> CandidateFacts {
    let session = cx.sess();
    CandidateFacts {
        harness: harness_status(session),
        provenance: provenance(item.span),
        context: item.context,
        body: body_presence(item.function),
        path: PathPolicy::InterimUnrestricted.select(&source_file(session, item.span)),
        documentation: documentation_status(item.attrs.iter().map(doc_attribute_shape)),
    }
}

/// Reports whether the session is compiling a test harness.
const fn harness_status(session: &Session) -> HarnessStatus {
    if session.opts.test {
        HarnessStatus::TestHarness
    } else {
        HarnessStatus::NotTestHarness
    }
}

/// Reports whether the item was authored or generated.
fn provenance(item_span: rustc_span::Span) -> Provenance {
    if item_span.from_expansion() {
        Provenance::Expansion
    } else {
        Provenance::Source
    }
}

/// Describes the item's source file for the driver's debug event.
///
/// Evaluated only when debug logging is enabled, so an inert run stays cheap
/// while still being observable to an operator who asks for it.
pub(super) fn source_label(session: &Session, item_span: rustc_span::Span) -> String {
    match source_file(session, item_span) {
        SourceFile::Real(path) => path.into_string(),
        SourceFile::Virtual => String::from("<virtual>"),
    }
}

/// Reports whether the function has a body.
const fn body_presence(function: &AstFn) -> BodyPresence {
    if function.body.is_some() {
        BodyPresence::Present
    } else {
        BodyPresence::Absent
    }
}

/// Recovers the item's source file from the compiler source map.
fn source_file(session: &Session, item_span: rustc_span::Span) -> SourceFile {
    match session.source_map().span_to_filename(item_span) {
        FileName::Real(real) => real
            .into_local_path()
            .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
            .map_or(SourceFile::Virtual, SourceFile::Real),
        FileName::CfgSpec(_)
        | FileName::Anon(_)
        | FileName::MacroExpansion(_)
        | FileName::ProcMacroSourceCode(_)
        | FileName::CliCrateAttr(_)
        | FileName::Custom(_)
        | FileName::DocTest(..)
        | FileName::InlineAsm(_) => SourceFile::Virtual,
    }
}
