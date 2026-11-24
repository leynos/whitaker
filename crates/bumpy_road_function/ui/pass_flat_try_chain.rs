//! Two separated runs of `?` must produce no diagnostic.
//!
//! rustc lowers `value?` into a `match` over `Try::branch`: a success arm that
//! binds the value and a residual arm that returns from the enclosing function.
//! Desugarings are transparent to the provenance classifier, so the generic
//! walk charged both arms a nesting depth and a control-flow weight. A flat
//! guard chain therefore accumulated two bumps and the diagnostic claimed
//! "clusters of nested conditional logic" in a function that contains no
//! nesting at all.
//!
//! Under ADR 0016 each `?` is charged as an early-return branch (1.5 at the
//! default weights) on its own line, so the chain is no longer free; with one
//! `?` per line it stays silent because six adjacent lines smooth to 1.5, under
//! the 2.5 threshold,
//! and CodeScene also scores this function 10.0. The function below is flat:
//! one `if`, and two separated runs of six `?`.

/// Join six path steps onto `root`, check the result, then join six more.
// @codescene(disable:"Bumpy Road Ahead") regression fixture: flat `?` must stay silent
fn join_twelve_steps(root: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let a = step("a")?;
    let b = step("b")?;
    let c = step("c")?;
    let d = step("d")?;
    let e = step("e")?;
    let f = step("f")?;

    let joined = root.join(&a).join(&b).join(&c).join(&d).join(&e).join(&f);

    if joined.as_os_str().is_empty() {
        return Err(String::from("empty join"));
    }

    let g = step("g")?;
    let h = step("h")?;
    let i = step("i")?;
    let j = step("j")?;
    let k = step("k")?;
    let l = step("l")?;

    Ok(joined.join(&g).join(&h).join(&i).join(&j).join(&k).join(&l))
}

/// Turn one path step into an owned component, rejecting the empty name.
fn step(what: &str) -> Result<std::path::PathBuf, String> {
    if what.is_empty() {
        Err(String::from("empty step"))
    } else {
        Ok(std::path::PathBuf::from(what))
    }
}

fn main() {
    let _ = join_twelve_steps(std::path::Path::new("root"));
}
