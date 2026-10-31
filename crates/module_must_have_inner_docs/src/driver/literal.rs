//! Track strings and comments while scanning Rust attribute text.
//!
//! This private scanner only protects meta-list delimiters from lexical
//! contents. Keep it local to `module_must_have_inner_docs`; it is not a Rust
//! parser or a general-purpose lexer.

#[derive(Default)]
pub(super) struct LexicalState {
    state: State,
    skipped_delimiter_byte: Option<usize>,
}

#[derive(Default)]
enum State {
    #[default]
    Outside,
    Quoted {
        is_escaped: bool,
    },
    Raw(RawStringState),
    LineComment,
    BlockComment {
        depth: usize,
    },
}

struct RawStringState {
    opening_quote: usize,
    hash_count: usize,
}

impl LexicalState {
    /// Track lexical contents and report whether they consume this character.
    pub(super) fn consumes(&mut self, input: &str, idx: usize, ch: char) -> bool {
        if self.skipped_delimiter_byte == Some(idx) {
            self.skipped_delimiter_byte = None;
            return true;
        }

        match std::mem::take(&mut self.state) {
            State::Outside => match state_at(input, idx, ch) {
                State::Outside => false,
                next_state => {
                    if matches!(next_state, State::BlockComment { .. }) {
                        self.skipped_delimiter_byte = Some(idx.saturating_add(1));
                    }
                    self.state = next_state;
                    true
                }
            },
            State::Quoted { is_escaped } => {
                self.state = next_quoted_state(is_escaped, ch);
                true
            }
            State::Raw(raw_string) => {
                self.state = raw_string.next(input, idx, ch);
                true
            }
            State::LineComment => {
                self.state = if ch == '\n' {
                    State::Outside
                } else {
                    State::LineComment
                };
                true
            }
            State::BlockComment { depth } => {
                self.state =
                    next_block_comment_state(input, idx, depth, &mut self.skipped_delimiter_byte);
                true
            }
        }
    }
}

/// Choose the lexical state that starts at the current character.
fn state_at(input: &str, idx: usize, ch: char) -> State {
    match (raw_string_start(input, idx), ch) {
        (Some((opening_quote, hash_count)), _) => State::Raw(RawStringState {
            opening_quote,
            hash_count,
        }),
        (None, '"') => State::Quoted { is_escaped: false },
        (None, '/') if input.get(idx..).is_some_and(|tail| tail.starts_with("//")) => {
            State::LineComment
        }
        (None, '/') if input.get(idx..).is_some_and(|tail| tail.starts_with("/*")) => {
            State::BlockComment { depth: 1 }
        }
        (None, _) => State::Outside,
    }
}

/// Advance a nested block-comment state and consume its delimiter pair.
fn next_block_comment_state(
    input: &str,
    idx: usize,
    depth: usize,
    skipped_delimiter_byte: &mut Option<usize>,
) -> State {
    let Some(suffix) = input.get(idx..) else {
        return State::BlockComment { depth };
    };
    if suffix.starts_with("/*") {
        *skipped_delimiter_byte = Some(idx.saturating_add(1));
        State::BlockComment {
            depth: depth.saturating_add(1),
        }
    } else if suffix.starts_with("*/") {
        *skipped_delimiter_byte = Some(idx.saturating_add(1));
        if depth == 1 {
            State::Outside
        } else {
            State::BlockComment {
                depth: depth.saturating_sub(1),
            }
        }
    } else {
        State::BlockComment { depth }
    }
}

/// Advance an escaped quoted string by one character.
fn next_quoted_state(is_escaped: bool, ch: char) -> State {
    match (is_escaped, ch) {
        (true, _) => State::Quoted { is_escaped: false },
        (false, '\\') => State::Quoted { is_escaped: true },
        (false, '"') => State::Outside,
        (false, _) => State::Quoted { is_escaped: false },
    }
}

impl RawStringState {
    /// Advance a raw string and exit only at its matching hash-delimited end.
    fn next(self, input: &str, idx: usize, ch: char) -> State {
        match (
            idx == self.opening_quote,
            ch == '"',
            raw_string_ends(input, idx, self.hash_count),
        ) {
            (false, true, true) => State::Outside,
            _ => State::Raw(self),
        }
    }
}

/// Find the opening quote and hash count for a raw string literal.
fn raw_string_start(input: &str, idx: usize) -> Option<(usize, usize)> {
    let suffix = input.get(idx..)?;
    let prefix_length = suffix
        .strip_prefix("br")
        .map(|_| 2)
        .or_else(|| suffix.strip_prefix('r').map(|_| 1))?;
    let raw_content = suffix.get(prefix_length..)?;
    let hash_count = raw_content.bytes().take_while(|byte| *byte == b'#').count();
    let opening_quote = idx.saturating_add(prefix_length).saturating_add(hash_count);

    raw_content
        .get(hash_count..)
        .filter(|tail| tail.starts_with('"'))
        .map(|_| (opening_quote, hash_count))
}

/// Check whether a quote is followed by the raw string's closing hashes.
fn raw_string_ends(input: &str, idx: usize, hash_count: usize) -> bool {
    input
        .get(idx.saturating_add(1)..)
        .and_then(|tail| tail.as_bytes().get(..hash_count))
        .is_some_and(|hashes| hashes.iter().all(|byte| *byte == b'#'))
}
