//! Kani-only samplers shared by the check proof suites.
//!
//! The flow and exit proofs both range over every repairable reason and host
//! fault. Keeping one pair of samplers here means both suites agree on what
//! "every variant" means, and deriving `kani::Arbitrary` on the enums keeps
//! that set equal to the declared one: a new variant needs no edit here, and
//! no wildcard arm can quietly stand in for a variant nobody samples.

use crate::ports::{HostFault, RepairableReason};

/// Pick an arbitrary repairable reason, including any variant added later.
pub(crate) fn any_reason() -> RepairableReason { kani::any() }

/// Pick an arbitrary host fault, including any variant added later.
pub(crate) fn any_fault() -> HostFault { kani::any() }
