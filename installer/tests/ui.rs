//! Compile-fail guards for the `sha2` 0.11 migration.
//!
//! `sha2` 0.11 made two source-breaking changes that the installer had to work
//! around:
//!
//! - `Sha256::finalize()` / `Sha256::digest()` now return
//!   `hybrid_array::Array<u8, _>`, which does not implement
//!   [`core::fmt::LowerHex`], so `format!("{:x}", digest)` no longer compiles.
//! - `Sha256` no longer implements [`std::io::Write`], so
//!   `io::copy(reader, &mut hasher)` no longer compiles.
//!
//! These `trybuild` cases pin those breaks: if a future change reintroduces the
//! pre-0.11 pattern — for example by downgrading `sha2` back to 0.10 — the
//! fixtures start compiling and this test fails, flagging the regression.

#[test]
fn sha2_0_11_pre_migration_patterns_fail_to_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/digest_lowerhex.rs");
    cases.compile_fail("tests/ui/hasher_io_write.rs");
}
