//! The facts a candidate function presents to the selection predicate.
//!
//! Each enum is a two- or five-variant classification rather than a `bool`, so
//! a truth table over the fact records is readable and an argument cannot be
//! silently transposed. Every enum derives the full comparison surface because
//! the exhaustive selection tests enumerate and snapshot these values.

use super::context::FunctionContext;

/// Whether rustc is compiling a test harness.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HarnessStatus {
    /// The session was invoked with `--test`.
    TestHarness,
    /// An ordinary library, binary, or metadata compilation.
    NotTestHarness,
}

/// Whether the item was written in the source or produced by a macro.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Provenance {
    /// The item's span has the root syntax context.
    Source,
    /// The item's span originates in an expansion.
    Expansion,
}

/// Whether the function has a body.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyPresence {
    /// The function has a body to run.
    Present,
    /// A bodiless declaration, such as a required trait method.
    Absent,
}

/// Whether the function's source file is selected by the path policy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PathSelection {
    /// The path policy selects this file.
    Selected,
    /// The path policy does not select this file.
    NotSelected,
}

/// Whether the function carries documentation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Documentation {
    /// At least one documentation attribute is present.
    Documented,
    /// No documentation attribute is present.
    Undocumented,
}

/// One candidate function reduced to the six facts the predicate reads.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CandidateFacts {
    /// Whether rustc is building a test harness.
    pub harness: HarnessStatus,
    /// Whether the item was authored or generated.
    pub provenance: Provenance,
    /// Where the function sits in the item tree.
    pub context: FunctionContext,
    /// Whether the function has a body.
    pub body: BodyPresence,
    /// Whether the path policy selects the source file.
    pub path: PathSelection,
    /// Whether the function carries documentation.
    pub documentation: Documentation,
}

/// Why a candidate was not diagnosed.
///
/// The variants are ordered as the gates are checked, so the reason reported is
/// always the first failing gate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SkipReason {
    /// The session is not compiling a test harness.
    NotTestHarness,
    /// The item was produced by a macro rather than written in the source.
    NotSourceAuthored,
    /// The function is nested somewhere the policy excludes.
    OutOfScopeContext,
    /// The function has no body.
    Bodyless,
    /// The source file is not selected by the path policy.
    PathNotSelected,
    /// The function already carries documentation.
    Documented,
}

/// The predicate's decision for one candidate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Verdict {
    /// Emit the missing-documentation diagnostic.
    Diagnose,
    /// Emit nothing, for the stated reason.
    Skip(SkipReason),
}
