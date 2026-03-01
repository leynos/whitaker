//! Incremental cognitive complexity computation with macro-expansion filtering.
//!
//! Provides [`CognitiveComplexityBuilder`], a pure library builder that
//! computes SonarSource-style cognitive complexity (CC) from incremental
//! method calls. Each call accepts `is_from_expansion: bool` so the HIR
//! walker (in the lint driver) can pass `span.from_expansion()` without
//! this module depending on `rustc_private`.
//!
//! Macro-expanded nodes are silently excluded from the CC score,
//! following the same pattern as
//! [`ForeignReferenceSet`](super::ForeignReferenceSet) and
//! [`MethodInfoBuilder`](crate::lcom4::MethodInfoBuilder).
//!
//! See `docs/brain-trust-lints-design.md` §Metric collection and
//! Clippy issue #14417 for the design rationale.

#[cfg(test)]
#[path = "cognitive_complexity_tests.rs"]
mod tests;

/// Incrementally computes cognitive complexity following SonarSource
/// rules, with macro-expansion filtering.
///
/// The HIR walker calls builder methods for each relevant node,
/// passing `is_from_expansion` (from `span.from_expansion()`). Nodes
/// where `is_from_expansion` is `true` are silently excluded from the
/// complexity count. Nesting depth is tracked internally.
///
/// # Three increment categories
///
/// - **Structural** (+1): `if`, `else if`, `else`, `match`, `for`,
///   `while`, `loop`, `?` operator, catch-equivalent constructs.
/// - **Nesting** (+effective_depth): applied alongside structural for
///   constructs that also incur a nesting penalty.
/// - **Fundamental** (+1): boolean operator sequence breaks (`&&`,
///   `||`).
///
/// # Examples
///
/// ```
/// use common::CognitiveComplexityBuilder;
///
/// let mut cc = CognitiveComplexityBuilder::new();
/// // Simulate: if condition { ... }
/// cc.record_structural_increment(false);  // +1
/// cc.record_nesting_increment(false);     // +0 (depth is 0)
/// cc.push_nesting(false);
/// // Simulate: nested if { ... }
/// cc.record_structural_increment(false);  // +1
/// cc.record_nesting_increment(false);     // +1 (depth is 1)
/// cc.push_nesting(false);
/// cc.pop_nesting();
/// cc.pop_nesting();
///
/// assert_eq!(cc.build(), 3);
/// ```
#[derive(Clone, Debug)]
pub struct CognitiveComplexityBuilder {
    score: usize,
    /// Each entry records whether that nesting level originated from a
    /// macro expansion (`true` = from expansion).
    nesting_stack: Vec<bool>,
    /// Count of non-expansion nesting levels currently on the stack.
    effective_depth: usize,
}

impl CognitiveComplexityBuilder {
    /// Creates a new builder with zero complexity and no nesting.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let cc = CognitiveComplexityBuilder::new();
    /// assert_eq!(cc.score(), 0);
    /// assert_eq!(cc.effective_depth(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            score: 0,
            nesting_stack: Vec::new(),
            effective_depth: 0,
        }
    }

    /// Records a structural increment (+1).
    ///
    /// Used for `if`, `else if`, `else`, `match` (the match keyword
    /// itself, not individual arms), `for`, `while`, `loop`, the `?`
    /// operator, and catch-equivalent constructs. Silently skipped
    /// when `is_from_expansion` is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// cc.record_structural_increment(false);
    /// assert_eq!(cc.score(), 1);
    ///
    /// cc.record_structural_increment(true); // macro — filtered
    /// assert_eq!(cc.score(), 1);
    /// ```
    pub fn record_structural_increment(&mut self, is_from_expansion: bool) {
        if !is_from_expansion {
            self.score += 1;
        }
    }

    /// Records a nesting increment (+effective_depth).
    ///
    /// Called alongside [`record_structural_increment`](Self::record_structural_increment)
    /// for constructs that also incur a nesting penalty (e.g. `if`,
    /// `match`, `for`, `while`, `loop`). The increment equals the
    /// current effective nesting depth, which excludes macro-expanded
    /// levels. Silently skipped when `is_from_expansion` is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// cc.push_nesting(false);
    /// cc.record_nesting_increment(false); // +1 (depth is 1)
    /// assert_eq!(cc.score(), 1);
    /// cc.pop_nesting();
    /// assert_eq!(cc.build(), 1);
    /// ```
    pub fn record_nesting_increment(&mut self, is_from_expansion: bool) {
        if !is_from_expansion {
            self.score += self.effective_depth;
        }
    }

    /// Records a fundamental increment (+1).
    ///
    /// Used for boolean operator sequence breaks (`&&`, `||`): each
    /// new sequence of the same operator or each switch between `&&`
    /// and `||` adds one increment. Silently skipped when
    /// `is_from_expansion` is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// cc.record_fundamental_increment(false);
    /// assert_eq!(cc.score(), 1);
    ///
    /// cc.record_fundamental_increment(true); // macro — filtered
    /// assert_eq!(cc.score(), 1);
    /// ```
    pub fn record_fundamental_increment(&mut self, is_from_expansion: bool) {
        if !is_from_expansion {
            self.score += 1;
        }
    }

    /// Enters a new nesting level.
    ///
    /// When `is_from_expansion` is `true`, the level is tracked for
    /// stack balance but does not increase the effective nesting depth
    /// used for nesting increments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// cc.push_nesting(false);
    /// assert_eq!(cc.effective_depth(), 1);
    ///
    /// cc.push_nesting(true); // macro — does not increase depth
    /// assert_eq!(cc.effective_depth(), 1);
    ///
    /// cc.pop_nesting();
    /// cc.pop_nesting();
    /// ```
    pub fn push_nesting(&mut self, is_from_expansion: bool) {
        self.nesting_stack.push(is_from_expansion);
        if !is_from_expansion {
            self.effective_depth += 1;
        }
    }

    /// Exits the most recent nesting level.
    ///
    /// If the popped level was not from a macro expansion, the
    /// effective depth is decremented.
    ///
    /// # Panics
    ///
    /// Panics if the nesting stack is empty, indicating a mismatched
    /// `push_nesting`/`pop_nesting` sequence in the caller.
    pub fn pop_nesting(&mut self) {
        match self.nesting_stack.pop() {
            Some(was_from_expansion) => {
                if !was_from_expansion {
                    self.effective_depth -= 1;
                }
            }
            None => panic!("pop_nesting called on an empty nesting stack"),
        }
    }

    /// Returns the current effective nesting depth (non-expansion
    /// levels only).
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// assert_eq!(cc.effective_depth(), 0);
    /// cc.push_nesting(false);
    /// assert_eq!(cc.effective_depth(), 1);
    /// cc.pop_nesting();
    /// ```
    #[must_use]
    pub fn effective_depth(&self) -> usize {
        self.effective_depth
    }

    /// Returns the accumulated cognitive complexity score so far.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let mut cc = CognitiveComplexityBuilder::new();
    /// cc.record_structural_increment(false);
    /// assert_eq!(cc.score(), 1);
    /// ```
    #[must_use]
    pub fn score(&self) -> usize {
        self.score
    }

    /// Consumes the builder and returns the final complexity score.
    ///
    /// # Panics
    ///
    /// Panics if the nesting stack is not empty, indicating unbalanced
    /// `push_nesting`/`pop_nesting` calls.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::CognitiveComplexityBuilder;
    ///
    /// let cc = CognitiveComplexityBuilder::new();
    /// assert_eq!(cc.build(), 0);
    /// ```
    #[must_use]
    pub fn build(self) -> usize {
        assert!(
            self.nesting_stack.is_empty(),
            "unbalanced nesting: {} levels remain on the stack",
            self.nesting_stack.len(),
        );
        self.score
    }
}

impl Default for CognitiveComplexityBuilder {
    fn default() -> Self {
        Self::new()
    }
}
