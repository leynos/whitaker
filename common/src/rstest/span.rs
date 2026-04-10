//! Pure helpers for recovering user-editable spans from macro-heavy frame chains.

/// A single span-recovery frame and whether it still comes from macro expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRecoveryFrame<T> {
    value: T,
    from_expansion: bool,
}

impl<T> SpanRecoveryFrame<T> {
    /// Builds a recovery frame from a value and expansion flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::SpanRecoveryFrame;
    /// use whitaker_common::span::{SourceLocation, SourceSpan};
    ///
    /// let span = SourceSpan::new(SourceLocation::new(3, 1), SourceLocation::new(3, 8))
    ///     .expect("example span should be valid");
    /// let frame = SpanRecoveryFrame::new(span, true);
    ///
    /// assert!(frame.from_expansion());
    /// ```
    #[must_use]
    pub const fn new(value: T, from_expansion: bool) -> Self {
        Self {
            value,
            from_expansion,
        }
    }

    /// Returns the stored frame value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Consumes the frame and returns the stored value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Returns whether the frame still originates from macro expansion.
    #[must_use]
    pub const fn from_expansion(&self) -> bool {
        self.from_expansion
    }
}

/// The result of recovering a user-editable span from an ordered frame chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserEditableSpan<T> {
    /// The original frame was already user-editable.
    Direct(T),
    /// A later frame recovered a user-editable span.
    Recovered(T),
    /// No frame in the chain pointed at user-editable code.
    MacroOnly,
}

impl<T> UserEditableSpan<T> {
    /// Converts the recovery result into an optional recovered value.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::UserEditableSpan;
    /// use whitaker_common::span::{SourceLocation, SourceSpan};
    ///
    /// let span = SourceSpan::new(SourceLocation::new(5, 1), SourceLocation::new(5, 7))
    ///     .expect("example span should be valid");
    ///
    /// assert_eq!(UserEditableSpan::Recovered(span).into_option(), Some(span));
    /// assert_eq!(UserEditableSpan::<SourceSpan>::MacroOnly.into_option(), None);
    /// ```
    #[must_use]
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Direct(value) | Self::Recovered(value) => Some(value),
            Self::MacroOnly => None,
        }
    }
}

/// Recovers the first user-editable frame from an ordered recovery chain.
///
/// The first frame represents the original location. Later frames are fallback
/// candidates such as macro invocation sites or expansion call-sites.
///
/// # Examples
///
/// ```
/// use whitaker_common::rstest::{SpanRecoveryFrame, UserEditableSpan, recover_user_editable_span};
/// use whitaker_common::span::{SourceLocation, SourceSpan};
///
/// let macro_span = SourceSpan::new(SourceLocation::new(2, 1), SourceLocation::new(2, 5))
///     .expect("example span should be valid");
/// let user_span = SourceSpan::new(SourceLocation::new(10, 1), SourceLocation::new(10, 12))
///     .expect("example span should be valid");
///
/// let recovered = recover_user_editable_span(&[
///     SpanRecoveryFrame::new(macro_span, true),
///     SpanRecoveryFrame::new(user_span, false),
/// ]);
///
/// assert_eq!(recovered, UserEditableSpan::Recovered(user_span));
/// ```
#[must_use]
pub fn recover_user_editable_span<T: Clone>(
    frames: &[SpanRecoveryFrame<T>],
) -> UserEditableSpan<T> {
    frames
        .iter()
        .enumerate()
        .find(|(_, frame)| !frame.from_expansion())
        .map_or(UserEditableSpan::MacroOnly, |(index, frame)| {
            if index == 0 {
                UserEditableSpan::Direct(frame.value().clone())
            } else {
                UserEditableSpan::Recovered(frame.value().clone())
            }
        })
}
