//! Builders for [`Location`] and [`Region`] objects.

use crate::model::location::{ArtifactLocation, Location, PhysicalLocation, Region};

/// Fluent builder for constructing a [`Region`].
///
/// Only `start_line` is required; all other fields default to `None`.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::RegionBuilder;
///
/// let region = RegionBuilder::new(10)
///     .with_end_line(15)
///     .build();
/// assert_eq!(region.start_line, 10);
/// assert_eq!(region.end_line, Some(15));
/// ```
#[derive(Debug, Clone)]
pub struct RegionBuilder {
    start_line: usize,
    start_column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    byte_offset: Option<usize>,
    byte_length: Option<usize>,
}

impl RegionBuilder {
    /// Creates a builder with the given 1-based start line.
    #[must_use]
    pub fn new(start_line: usize) -> Self {
        Self {
            start_line,
            start_column: None,
            end_line: None,
            end_column: None,
            byte_offset: None,
            byte_length: None,
        }
    }

    /// Sets the 1-based start column.
    #[must_use]
    pub fn with_start_column(mut self, col: usize) -> Self {
        self.start_column = Some(col);
        self
    }

    /// Sets the 1-based end line.
    #[must_use]
    pub fn with_end_line(mut self, line: usize) -> Self {
        self.end_line = Some(line);
        self
    }

    /// Sets the 1-based end column.
    #[must_use]
    pub fn with_end_column(mut self, col: usize) -> Self {
        self.end_column = Some(col);
        self
    }

    /// Sets the byte offset from the start of the artifact.
    #[must_use]
    pub fn with_byte_offset(mut self, offset: usize) -> Self {
        self.byte_offset = Some(offset);
        self
    }

    /// Sets the byte length.
    #[must_use]
    pub fn with_byte_length(mut self, length: usize) -> Self {
        self.byte_length = Some(length);
        self
    }

    /// Consumes the builder and produces a [`Region`].
    #[must_use]
    pub fn build(self) -> Region {
        Region {
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
            byte_offset: self.byte_offset,
            byte_length: self.byte_length,
        }
    }
}

/// Fluent builder for constructing a [`Location`].
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{LocationBuilder, RegionBuilder};
///
/// let loc = LocationBuilder::new("src/main.rs")
///     .with_region(RegionBuilder::new(10).with_end_line(15).build())
///     .build();
/// assert_eq!(loc.physical_location.artifact_location.uri, "src/main.rs");
/// ```
#[derive(Debug, Clone)]
pub struct LocationBuilder {
    uri: String,
    uri_base_id: Option<String>,
    region: Option<Region>,
}

impl LocationBuilder {
    /// Creates a builder for a location at the given file URI.
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            uri_base_id: None,
            region: None,
        }
    }

    /// Sets the base identifier for resolving relative URIs.
    #[must_use]
    pub fn with_uri_base_id(mut self, base: impl Into<String>) -> Self {
        self.uri_base_id = Some(base.into());
        self
    }

    /// Sets the region within the artifact.
    #[must_use]
    pub fn with_region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    /// Consumes the builder and produces a [`Location`].
    #[must_use]
    pub fn build(self) -> Location {
        Location {
            physical_location: PhysicalLocation {
                artifact_location: ArtifactLocation {
                    uri: self.uri,
                    uri_base_id: self.uri_base_id,
                },
                region: self.region,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_builder_minimal() {
        let region = RegionBuilder::new(5).build();
        assert_eq!(region.start_line, 5);
        assert!(region.start_column.is_none());
        assert!(region.end_line.is_none());
    }

    #[test]
    fn region_builder_full() {
        let region = RegionBuilder::new(1)
            .with_start_column(5)
            .with_end_line(10)
            .with_end_column(20)
            .with_byte_offset(100)
            .with_byte_length(200)
            .build();
        assert_eq!(region.start_line, 1);
        assert_eq!(region.start_column, Some(5));
        assert_eq!(region.end_line, Some(10));
        assert_eq!(region.end_column, Some(20));
        assert_eq!(region.byte_offset, Some(100));
        assert_eq!(region.byte_length, Some(200));
    }

    #[test]
    fn location_builder_minimal() {
        let loc = LocationBuilder::new("src/main.rs").build();
        assert_eq!(loc.physical_location.artifact_location.uri, "src/main.rs");
        assert!(loc.physical_location.region.is_none());
    }

    #[test]
    fn location_builder_with_region() {
        let loc = LocationBuilder::new("src/lib.rs")
            .with_region(RegionBuilder::new(42).build())
            .build();
        let region = loc.physical_location.region.as_ref().expect("region");
        assert_eq!(region.start_line, 42);
    }

    #[test]
    fn location_builder_with_base_id() {
        let loc = LocationBuilder::new("src/lib.rs")
            .with_uri_base_id("%SRCROOT%")
            .build();
        assert_eq!(
            loc.physical_location
                .artifact_location
                .uri_base_id
                .as_deref(),
            Some("%SRCROOT%")
        );
    }
}
