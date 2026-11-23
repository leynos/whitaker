//! Compiler-free candidate-selection kernel for the lint.
//!
//! The driver reduces one AST item to a set of [`CandidateFacts`] and asks
//! [`select`] whether to diagnose. Keeping the decision here means the contract
//! can be enumerated over every combination of facts without linking rustc.
//!
//! The predicate is a conjunction of six gates, evaluated in a fixed order:
//! harness, provenance, context, body, path, and documentation. The first gate
//! that fails names the reason the candidate was skipped, which makes gate
//! precedence a testable property rather than an accident of statement order.

mod context;
mod documentation;
mod facts;
mod selection;

pub use context::{ContainerKind, ContainerStack, FunctionContext, FunctionShape};
pub use documentation::{DocAttributeShape, documentation_status};
pub use facts::{
    BodyPresence,
    CandidateFacts,
    Documentation,
    HarnessStatus,
    PathSelection,
    Provenance,
    SkipReason,
    Verdict,
};
pub use selection::select;

#[cfg(test)]
#[path = "selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "documentation_tests.rs"]
mod documentation_tests;

#[cfg(test)]
#[path = "context_tests.rs"]
mod context_tests;

#[cfg(test)]
#[path = "context_props.rs"]
mod context_props;
