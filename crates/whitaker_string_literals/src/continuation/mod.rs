//! Finds source-level continuation escapes and records their byte ranges.

use core::ops::Range;

use crate::facts::{BodyOffset, LiteralBody};

#[cfg(kani)]
mod kani;
#[cfg(test)]
mod props;
#[cfg(test)]
mod tests;

/// One cooked-string continuation and the source bytes Rust consumes for it.
///
/// The ranges are relative to the [`LiteralBody`] passed to [`LiteralBody::scan`].
/// `escape_range` includes the unpaired backslash and LF; the whitespace range
/// starts immediately after that LF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Continuation {
    /// The unpaired backslash and its following LF.
    pub escape_range: Range<BodyOffset>,
    /// The whitespace consumed after the LF, possibly empty.
    pub skipped_whitespace_range: Range<BodyOffset>,
    /// Whether this boundary joins source fragments or expresses layout trim.
    pub kind: ContinuationKind,
}

/// The source-content role of one continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationKind {
    /// Source content occurs before and after the continuation.
    Join,
    /// No non-space or tab source byte precedes the continuation on its line.
    LeadingLayoutTrim,
    /// The continuation consumes the rest of the literal body.
    TrailingLayoutTrim,
}

impl LiteralBody<'_> {
    /// Finds continuation escapes in the cooked literal body.
    ///
    /// A continuation requires an odd run of backslashes immediately before
    /// LF. Rust consumes the unpaired backslash, LF, and all following spaces,
    /// tabs, LFs, and CRs. Formfeed is not consumed.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::{continuation::ContinuationKind, facts::LiteralBody};
    ///
    /// let body = LiteralBody::new("alpha \\\nbeta");
    /// let continuations = body.scan();
    /// assert_eq!(continuations.len(), 1);
    /// assert_eq!(continuations[0].kind, ContinuationKind::Join);
    /// ```
    #[must_use]
    pub fn scan(self) -> Vec<Continuation> {
        let mut continuations = Vec::new();
        scan_into(self.as_str(), |continuation| {
            continuations.push(continuation);
        });
        continuations
    }
}

fn scan_into(body: &str, mut emit: impl FnMut(Continuation)) {
    let bytes = body.as_bytes();
    let mut cursor = 0;
    let mut line_has_content = false;

    while let Some(&byte) = bytes.get(cursor) {
        if byte == b'\\' {
            (cursor, line_has_content) =
                scan_backslash_run(bytes, cursor, line_has_content, &mut emit);
        } else {
            line_has_content = line_content_after_byte(byte, line_has_content);
            cursor = cursor.saturating_add(1);
        }
    }
}

fn scan_backslash_run(
    bytes: &[u8],
    run_start: usize,
    line_has_content: bool,
    emit: &mut impl FnMut(Continuation),
) -> (usize, bool) {
    let mut run_end = run_start;
    while bytes.get(run_end) == Some(&b'\\') {
        run_end = run_end.saturating_add(1);
    }

    match continuation_after_backslash_run(bytes, run_start, run_end, line_has_content) {
        Some((continuation, next_cursor)) => {
            emit(continuation);
            (next_cursor, false)
        }
        None => (run_end, true),
    }
}

const fn line_content_after_byte(byte: u8, line_has_content: bool) -> bool {
    match byte {
        b'\n' => false,
        b' ' | b'\t' => line_has_content,
        _ => true,
    }
}

fn continuation_after_backslash_run(
    bytes: &[u8],
    run_start: usize,
    run_end: usize,
    line_has_content: bool,
) -> Option<(Continuation, usize)> {
    let run_length = run_end.checked_sub(run_start)?;
    if bytes.get(run_end) != Some(&b'\n') || run_length & 1 != 1 {
        return None;
    }

    let escape_start = run_end.checked_sub(1)?;
    let after_newline = run_end.checked_add(1)?;
    let mut cursor = after_newline;
    while bytes
        .get(cursor)
        .is_some_and(|candidate| is_consumed_whitespace(*candidate))
    {
        cursor = cursor.saturating_add(1);
    }

    let has_content_before = line_has_content || run_length > 1;
    let has_content_after = bytes.get(cursor).is_some();
    let kind = match (has_content_before, has_content_after) {
        (true, true) => ContinuationKind::Join,
        (false, _) => ContinuationKind::LeadingLayoutTrim,
        (true, false) => ContinuationKind::TrailingLayoutTrim,
    };
    let escape_end = BodyOffset::from_index(after_newline);
    Some((
        Continuation {
            escape_range: BodyOffset::from_index(escape_start)..escape_end,
            skipped_whitespace_range: escape_end..BodyOffset::from_index(cursor),
            kind,
        },
        cursor,
    ))
}

const fn is_consumed_whitespace(byte: u8) -> bool { matches!(byte, b' ' | b'\t' | b'\n' | b'\r') }
