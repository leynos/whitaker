//! How loudly an implicit repair reports progress, as a port value.
//!
//! The root command resolves `-q`/`-v` into its own diagnostic policy, but the
//! port must not name that policy type: it belongs to the binary's `tracing`
//! installation, which is an adapter-side concern. This value carries only the
//! two facts the installer's arguments need, so the port stays free of the
//! diagnostics module and a caller cannot smuggle a subscriber through it.

/// The verbosity an implicit repair hands to the installer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RepairVerbosity {
    /// Suppress the installer's output below `ERROR`.
    pub quiet: bool,
    /// How many times `--verbose` was given; any non-zero count enables debug.
    pub verbosity: u8,
}
