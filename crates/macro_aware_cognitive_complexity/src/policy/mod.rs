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
//! - The return count gains one for each authored `return` whose previous authored expression is
//!   not a return. A generated event is discarded before any state changes, so it neither charges
//!   nor breaks return adjacency, and a guard charge is not an expression, so a `return` that
//!   guards an arm stays adjacent to the `return` ending the arm before it — the order pinned
//!   Clippy's `prev_expr` sees.
//! - The adjustment is the return count for a body whose result type is `Result`, and half of it
//!   (truncating) otherwise. A `return` inside a transparent block is credited by the type of that
//!   block, not the root's: the two counts are credited separately and summed.
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
    ///
    /// Pinned Clippy counts a guard while visiting the enclosing `match` and
    /// tracks return adjacency over the expressions it visits, so a guard is a
    /// charge but not an expression: it must not break return adjacency.
    Guard,
    /// A `return` expression in the selected body itself.
    ///
    /// Whether it earns the full or the halved credit is decided by the
    /// `returns_result` argument of [`score`].
    Return,
    /// A `return` expression inside a transparent closure or `async` block.
    ///
    /// The block owns the `return`, so the block's own result type decides its
    /// credit whatever the selected body's type is. `#[async_trait]` turns a
    /// `Result` method into one that returns a boxed future, and a `Result`
    /// block elsewhere in a function says nothing about that function's
    /// `return`s.
    BlockReturn {
        /// Whether the owning block yields a `core::result::Result`.
        returns_result: bool,
    },
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

impl Observed {
    /// Report whether this event is a `return`, in the body or in a block.
    #[must_use]
    pub const fn is_return(self) -> bool { matches!(self, Self::Return | Self::BlockReturn { .. }) }
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
/// add charges nor break return adjacency. A guard charge does not break
/// adjacency either, because it is not an expression in the traversal. The
/// adjustment depends on whether the selected body's result type is `Result`.
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
        returns: charges.returns.total(),
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
    returns: Returns,
}

/// Counted returns, split by the type that decides their credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Returns {
    /// Returns in the selected body, credited by the body's own type.
    body: u64,
    /// Returns owned by a `Result`-typed transparent block.
    result_block: u64,
    /// Returns owned by any other transparent block.
    other_block: u64,
}

impl Returns {
    /// The returns counted in all, however they are credited.
    const fn total(self) -> u64 {
        self.body
            .saturating_add(self.result_block)
            .saturating_add(self.other_block)
    }
}

/// Accumulate charges from authored events while preserving return adjacency.
fn authored_charges(events: &[Event]) -> Charges {
    let mut charges = Charges {
        branches: 1,
        returns: Returns {
            body: 0,
            result_block: 0,
            other_block: 0,
        },
    };
    let mut previous_was_return = false;

    for event in events.iter().filter(|event| !event.origin.is_generated()) {
        charge(&mut charges, &mut previous_was_return, *event);
    }

    charges
}

/// Fold one authored event into the running charges and adjacency state.
///
/// A guard charge is folded into the branch count but leaves adjacency alone.
/// Pinned Clippy adds a guard's charge while visiting the enclosing `match` and
/// decides adjacency from the expressions it visits, so a guard's own `return`
/// is adjacent to the `return` that ends the arm before it rather than
/// separated by the guard.
fn charge(charges: &mut Charges, previous_was_return: &mut bool, event: Event) {
    match event.observed {
        Observed::Branch | Observed::Guard => {
            charges.branches = charges.branches.saturating_add(1);
        }
        Observed::Return if !*previous_was_return => {
            charges.returns.body = charges.returns.body.saturating_add(1);
        }
        Observed::BlockReturn { returns_result } if !*previous_was_return => {
            let count = if returns_result {
                &mut charges.returns.result_block
            } else {
                &mut charges.returns.other_block
            };
            *count = count.saturating_add(1);
        }
        Observed::Return | Observed::BlockReturn { .. } | Observed::Neutral => {}
    }

    if event.observed != Observed::Guard {
        *previous_was_return = event.observed.is_return();
    }
}

/// Credit `Result` returns in full and halve the rest.
///
/// The selected body's own returns join whichever side its type puts them on.
const fn return_adjustment(returns: Returns, returns_result: bool) -> u64 {
    let (full, halved) = if returns_result {
        (
            returns.body.saturating_add(returns.result_block),
            returns.other_block,
        )
    } else {
        (
            returns.result_block,
            returns.body.saturating_add(returns.other_block),
        )
    };
    // Euclidean division matches truncation for unsigned values.
    full.saturating_add(halved.div_euclid(2))
}
