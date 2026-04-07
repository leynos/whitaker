//! UI fixture that should trigger the bumpy road lint.
//!
//! This variant uses a match expression with two arms, each containing a nested
//! conditional block. The two conditional clusters form separated bumps.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Debug,
    Release,
}

impl Mode {
    fn is_debug(self) -> bool {
        matches!(self, Self::Debug)
    }
}

const MIN_LEN: usize = 64;

/// Reads key material from disk and applies mode-dependent validation.
///
/// The match arms each contain nested conditional blocks, producing two
/// separated complexity bumps.
///
/// ```ignore
/// key_from_file(Mode::Debug, true);
/// ```
pub fn key_from_file(mode: Mode, allow_fallback: bool) -> Result<Vec<u8>, String> {
    let path = PathBuf::from("/tmp/key");

    match std::fs::read(&path) {
        Ok(mut bytes) => {
            let length = bytes.len();
            if mode == Mode::Release && length < MIN_LEN {
                bytes.fill(0);
                return Err(format!(
                    "key at {} is too short ({length} < {MIN_LEN})",
                    path.display()
                ));
            }
            let result = bytes.clone();
            bytes.fill(0);
            Ok(result)
        }
        Err(error) => {
            if mode.is_debug() || allow_fallback {
                Ok(vec![0; MIN_LEN])
            } else {
                Err(format!(
                    "cannot read key from {}: {error}",
                    path.display()
                ))
            }
        }
    }
}

fn dead_code_fixture_marker() {}

fn main() {}
