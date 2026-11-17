//! One-region and zero-region reductions of the issue #357 fixture.
//!
//! `ui/pass_two_shallow_clusters.rs` holds the full two-region function. RFC
//! 0013 describes three manual controls that remove the prefix-validation
//! match, the common-prefix loop's inner `if`, or both. Each is a function
//! here so a single CodeScene review reports all of them by name.

/// Keep the prefix-validation match and drop the loop's inner `if`.
fn without_loop_if<'a>(path: &'a [&'a str], manifest: &[&str]) -> Result<&'a str, ()> {
    match (path.first(), manifest.first()) {
        (Some(path_prefix), Some(manifest_prefix)) => {
            if path_prefix != manifest_prefix {
                return Err(());
            }
        }
        _ => {}
    }

    let mut common = 0;
    while let (Some(_), Some(_)) = (path.get(common), manifest.get(common)) {
        common += 1;
    }

    let Some(text) = path.get(common) else {
        return Err(());
    };
    Ok(text)
}

/// Keep the common-prefix loop and drop the prefix-validation match.
fn without_prefix_match<'a>(path: &'a [&'a str], manifest: &[&str]) -> Result<&'a str, ()> {
    let mut common = 0;
    while let (Some(path_component), Some(manifest_component)) =
        (path.get(common), manifest.get(common))
    {
        if path_component != manifest_component {
            break;
        }
        common += 1;
    }

    let Some(text) = path.get(common) else {
        return Err(());
    };
    Ok(text)
}

/// Drop both nested regions, keeping only the trailing `let ... else`.
fn without_either<'a>(path: &'a [&'a str], manifest: &[&str]) -> Result<&'a str, ()> {
    let common = path
        .iter()
        .zip(manifest.iter())
        .take_while(|(path_component, manifest_component)| path_component == manifest_component)
        .count();

    let Some(text) = path.get(common) else {
        return Err(());
    };
    Ok(text)
}

/// Call each function so the fixture compiles without dead-code warnings.
fn main() {
    let path = ["prefix", "target", "file"];
    let manifest = ["prefix", "manifest"];
    let _ = (
        without_loop_if(&path, &manifest),
        without_prefix_match(&path, &manifest),
        without_either(&path, &manifest),
    );
}
