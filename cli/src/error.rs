//! Shared typed errors for application configuration sources.

use std::sync::Arc;

use ortho_config::OrthoError;

/// Report an invalid named configuration value.
pub(crate) fn invalid(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}
