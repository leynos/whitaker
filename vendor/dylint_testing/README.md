# Local Dylint testing compatibility patch

This directory contains `dylint_testing` 6.0.4 from crates.io, with one small
compatibility change for managed Rust compiler wrappers.

Upstream issue [1696](https://github.com/trailofbits/dylint/issues/1696)
records that the UI harness assumes verbose Cargo output begins with `rustc`.
Managed wrappers put the real compiler later in the command. This patch locates
that compiler and keeps its arguments, while allowing the wrapper to continue
running.

Remove this patch when an upstream release handles wrapped compiler commands.
The upstream licence is MIT OR Apache-2.0; this vendored copy uses the MIT
option in [LICENSE-MIT](LICENSE-MIT).
