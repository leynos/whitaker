//! Reproduce the issue #357 non-finding with two shallow nested regions.

/// Return the first path component after the common prefix.
///
/// # Errors
///
/// Return an error when the path prefixes differ or the paths are equal.
// @codescene(disable:"Bumpy Road Ahead") issue #357 fixture (2026-11-08)
fn relative_to_manifest<'a>(path: &'a [&'a str], manifest: &[&str]) -> Result<&'a str, ()> {
    match (path.first(), manifest.first()) {
        (Some(path_prefix), Some(manifest_prefix)) => {
            if path_prefix != manifest_prefix {
                return Err(());
            }
        }
        _ => {}
    }

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

/// Exercise the fixture without adding another conditional region.
fn main() {
    let path = ["prefix", "target", "file"];
    let manifest = ["prefix", "manifest"];
    let _ = relative_to_manifest(&path, &manifest);
}
