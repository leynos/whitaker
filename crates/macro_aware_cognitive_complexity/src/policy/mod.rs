//! Compiler-free scoring kernel for the macro-aware complexity lint.
//!
//! The driver reduces one selected body to an ordered stream of [`Event`]s and
//! asks this module for a [`Score`]. Keeping the arithmetic here means the
//! contract can be enumerated and property-tested without linking rustc.
//!
//! The arithmetic mirrors pinned Clippy over authored nodes only:
//!
//! - The branch count starts at one and gains one for each authored branch (`if` or multi-arm
//!   `match`) and each authored match-arm guard.
//! - The return count gains one for each authored `return` whose previous authored event is not a
//!   return. A generated event is discarded before any state changes, so it neither charges nor
//!   breaks return adjacency.
//! - The adjustment is the return count for a body whose result type is `Result`, and half of it
//!   (truncating) otherwise.
//! - The total is `branches - adjustment`, or `branches` when the adjustment would exceed it. A
//!   total of zero is therefore possible.

use whitaker_common::node_origin::NodeOrigin;

#[cfg(test)]
mod tests;

/// One charge category observed while walking a selected body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observed {
    /// An authored `if` expression or a multi-arm `match` expression.
    Branch,
    /// One match-arm guard, charged independently of its match.
    Guard,
    /// A `return` expression.
    Return,
    /// Any other visited expression, recorded only for return adjacency.
    Neutral,
}

/// One visited expression with its provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    /// The charge category this expression contributes.
    pub observed: Observed,
    /// Whether a person authored the expression or a macro generated it.
    pub origin: NodeOrigin,
}

impl Event {
    /// Build one visited expression from its charge category and provenance.
    ///
    /// # Examples
    ///
    /// ```
    /// use macro_aware_cognitive_complexity::policy::{Event, Observed};
    /// use whitaker_common::node_origin::NodeOrigin;
    ///
    /// let event = Event::new(Observed::Branch, NodeOrigin::Authored);
    /// assert_eq!(event.observed, Observed::Branch);
    /// assert!(!event.origin.is_generated());
    /// ```
    #[must_use]
    pub const fn new(observed: Observed, origin: NodeOrigin) -> Self { Self { observed, origin } }
}

/// The arithmetic decomposition of a complexity score.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Score {
    /// The baseline one plus authored branches and guards.
    pub branches: u64,
    /// Authored returns that follow a non-return authored expression.
    pub returns: u64,
    /// The returns credited against the branch count.
    pub adjustment: u64,
    /// The reported complexity, `branches - adjustment` or `branches`.
    pub total: u64,
}

impl Score {
    /// Report whether this score strictly exceeds the configured limit.
    ///
    /// Clippy compares `complexity > limit`, so a score equal to the limit is
    /// silent.
    ///
    /// # Examples
    ///
    /// ```
    /// use macro_aware_cognitive_complexity::policy::Score;
    ///
    /// let score = Score {
    ///     branches: 2,
    ///     returns: 0,
    ///     adjustment: 0,
    ///     total: 2,
    /// };
    /// assert!(score.exceeds(1));
    /// assert!(!score.exceeds(2));
    /// ```
    #[must_use]
    pub const fn exceeds(self, limit: u64) -> bool { self.total > limit }
}

/// Score one selected body's ordered event stream.
///
/// Generated events are discarded before any state changes, so they neither
/// add charges nor break return adjacency. The adjustment depends on whether
/// the selected body's result type is `Result`.
///
/// # Examples
///
/// ```
/// use macro_aware_cognitive_complexity::policy::{Event, Observed, score};
/// use whitaker_common::node_origin::NodeOrigin;
///
/// let events = [
///     Event::new(Observed::Branch, NodeOrigin::Authored),
///     Event::new(Observed::Return, NodeOrigin::Generated),
/// ];
/// let score = score(&events, false);
/// assert_eq!(score.branches, 2);
/// assert_eq!(score.returns, 0);
/// assert_eq!(score.total, 2);
/// ```
#[must_use]
pub fn score(events: &[Event], returns_result: bool) -> Score {
    let charges = authored_charges(events);
    let adjustment = return_adjustment(charges.returns, returns_result);
    let total = if charges.branches >= adjustment {
        charges.branches - adjustment
    } else {
        charges.branches
    };

    Score {
        branches: charges.branches,
        returns: charges.returns,
        adjustment,
        total,
    }
}

/// Authored charge counts collected before the adjustment is applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Charges {
    /// The baseline one plus authored branches and guards.
    branches: u64,
    /// Authored returns whose previous authored event was not a return.
    returns: u64,
}

/// Accumulate charges from authored events while preserving return adjacency.
fn authored_charges(events: &[Event]) -> Charges {
    let mut charges = Charges {
        branches: 1,
        returns: 0,
    };
    let mut previous_was_return = false;

    for event in events.iter().filter(|event| !event.origin.is_generated()) {
        charge(&mut charges, &mut previous_was_return, *event);
    }

    charges
}

/// Fold one authored event into the running charges and adjacency state.
fn charge(charges: &mut Charges, previous_was_return: &mut bool, event: Event) {
    match event.observed {
        Observed::Branch | Observed::Guard => {
            charges.branches = charges.branches.saturating_add(1);
        }
        Observed::Return if !*previous_was_return => {
            charges.returns = charges.returns.saturating_add(1);
        }
        Observed::Return | Observed::Neutral => {}
    }

    *previous_was_return = event.observed == Observed::Return;
}

/// Credit returns in full for a `Result` body and halve them otherwise.
const fn return_adjustment(returns: u64, returns_result: bool) -> u64 {
    if returns_result {
        returns
    } else {
        // Euclidean division matches truncation for unsigned values.
        returns.div_euclid(2)
    }
}
