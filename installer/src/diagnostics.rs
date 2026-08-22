//! Runtime diagnostics configuration for the installer binary.
//!
//! The binary installs one stderr tracing subscriber so operational Git and
//! workspace events are available through the standard `RUST_LOG` filter.

use tracing_subscriber::EnvFilter;

/// Installs the process-wide installer diagnostics subscriber.
pub(super) fn initialize() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
