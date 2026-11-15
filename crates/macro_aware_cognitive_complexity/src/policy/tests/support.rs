//! Event constructors shared by the scoring tests.

use whitaker_common::node_origin::NodeOrigin;

use crate::policy::{Event, Observed};

/// Build an event written by a person.
pub(super) const fn authored(observed: Observed) -> Event {
    Event::new(observed, NodeOrigin::Authored)
}

/// Build an event produced by a macro expansion or compiler pass.
pub(super) const fn generated(observed: Observed) -> Event {
    Event::new(observed, NodeOrigin::Generated)
}
