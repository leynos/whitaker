//! SARIF 2.1.0 data model types.
//!
//! This module contains the Rust representations of the SARIF 2.1.0
//! specification subset used by Whitaker. Types are organized by concept:
//!
//! - [`log`] — top-level [`SarifLog`] container.
//! - [`run`] — [`Run`], [`Tool`], [`ToolComponent`], [`Invocation`], and
//!   [`Artefact`].
//! - [`result`] — [`SarifResult`], [`Level`], and [`Message`].
//! - [`location`] — [`Location`], [`PhysicalLocation`],
//!   [`ArtefactLocation`], [`Region`], and [`RelatedLocation`].
//! - [`descriptor`] — [`ReportingDescriptor`] and
//!   [`MultiformatMessageString`].
//!
//! All types implement `Serialize` and `Deserialize` with `camelCase` field
//! naming to match the SARIF JSON schema.

pub mod descriptor;
pub mod location;
pub mod log;
pub mod result;
pub mod run;

pub use descriptor::{MultiformatMessageString, ReportingDescriptor};
pub use location::{ArtefactLocation, Location, PhysicalLocation, Region, RelatedLocation};
pub use log::SarifLog;
pub use result::{Level, Message, SarifResult};
pub use run::{Artefact, Invocation, Run, Tool, ToolComponent};
