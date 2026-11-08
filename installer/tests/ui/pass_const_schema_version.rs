//! Compile-time use of public schema-version const methods.

use whitaker_installer::artefact::{
    schema_version::SchemaVersion,
    target::TargetTriple,
};
use whitaker_installer::dependency_binaries::{manifest_contents, provenance_filename};

const CURRENT_SCHEMA_VERSION: u32 = SchemaVersion::current().as_u32();
const SUPPORTED_TARGET_COUNT: usize = TargetTriple::supported().len();
const _: &str = provenance_filename();
const _: &str = manifest_contents();
const _: () = assert!(CURRENT_SCHEMA_VERSION == 1 && SUPPORTED_TARGET_COUNT > 0);

fn main() {}
