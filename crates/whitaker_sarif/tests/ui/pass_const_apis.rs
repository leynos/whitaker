//! Compile-pass checks for const SARIF builder APIs in an external crate.

use whitaker_sarif::{Level, LocationBuilder, Region, RegionBuilder, ResultBuilder};
use whitaker_sarif::WhitakerPropertiesBuilder;

const REGION_BUILDER: RegionBuilder = RegionBuilder::new(10)
    .with_start_column(2)
    .with_end_line(15)
    .with_end_column(4)
    .with_byte_offset(20)
    .with_byte_length(5);

/// Apply the location builder's region setter in a const context.
const fn with_region(builder: LocationBuilder, region: Region) -> LocationBuilder {
    builder.with_region(region)
}

/// Apply the result builder's level setter in a const context.
const fn with_level(builder: ResultBuilder) -> ResultBuilder {
    builder.with_level(Level::Warning)
}

/// Apply the properties builder's numeric setters in a const context.
const fn with_properties(builder: WhitakerPropertiesBuilder) -> WhitakerPropertiesBuilder {
    builder
        .with_k(5)
        .with_window(8)
        .with_jaccard(0.8)
        .with_cosine(0.9)
        .with_group_id(2)
        .with_class_size(3)
}

/// Assert that the public const builder methods work in an external crate.
fn main() {
    let region = REGION_BUILDER
        .build()
        .unwrap_or_else(|_| panic!("const region builder should be valid"));
    assert_eq!(region.start_line, 10);
    assert_eq!(region.start_column, Some(2));
    assert_eq!(region.end_line, Some(15));
    assert_eq!(region.end_column, Some(4));
    assert_eq!(region.byte_offset, Some(20));
    assert_eq!(region.byte_length, Some(5));
    let _ = with_region as fn(LocationBuilder, Region) -> LocationBuilder;
    let _ = with_level as fn(ResultBuilder) -> ResultBuilder;
    let _ = with_properties as fn(WhitakerPropertiesBuilder) -> WhitakerPropertiesBuilder;
}
