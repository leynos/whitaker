//! Guard adjacency fixture: a guard charge must not break return adjacency.
//!
//! The second arm's guard is itself a bare `return`, so the only event between
//! the first arm's `return` and the guard's is the guard charge: the stream is
//! `return`, `guard`, `return`. A guard charge is not an expression, so the
//! two returns stay adjacent and only the first is credited: `branches = 3`
//! (baseline, the three-arm match, the guard) with `returns = 1` gives a total
//! of three, which the limit of two reports. If a guard charge broke adjacency
//! instead, the guard's return would be credited, `returns` would be two, the
//! adjustment one, and the total two, which that limit would not report. The
//! fixture therefore fails if the regression returns.

pub mod fixture {
    //! One match whose guard returns between two other returns.

    #[expect(
        unreachable_code,
        reason = "the diverging guard is the shape under test"
    )]
    pub fn guarded_returns(value: Option<u8>) {
        match value {
            None => return,
            Some(_) if return => (),
            Some(_) => (),
        }
    }
}

fn main() { fixture::guarded_returns(None); }