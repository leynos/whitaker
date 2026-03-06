//! Builders for [`Location`] and [`Region`] objects.

use crate::error::SarifError;
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
///     .build()
///     .expect("valid region");
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
    ///
    /// # Errors
    ///
    /// Returns [`SarifError::InvalidRegion`] if `start_line` is zero, if
    /// `start_column` or `end_column` is zero, if `end_line` is less than
    /// `start_line`, or if both columns are set and `end_column` is less
    /// than `start_column` on the same line (either explicit or implicit).
    pub fn build(self) -> crate::error::Result<Region> {
        if self.start_line < 1 {
            return Err(SarifError::InvalidRegion("start_line must be >= 1".into()));
        }
        if let Some(sc) = self.start_column
            && sc < 1
        {
            return Err(SarifError::InvalidRegion(
                "start_column must be >= 1".into(),
            ));
        }
        if let Some(ec) = self.end_column
            && ec < 1
        {
            return Err(SarifError::InvalidRegion("end_column must be >= 1".into()));
        }
        if let Some(end_line) = self.end_line {
            if end_line < self.start_line {
                return Err(SarifError::InvalidRegion(format!(
                    "end_line ({end_line}) must be >= start_line ({})",
                    self.start_line
                )));
            }
            if end_line == self.start_line {
                self.validate_same_line_columns()?;
            }
        } else {
            // When end_line is None the region is implicitly single-line.
            self.validate_same_line_columns()?;
        }
        Ok(Region {
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
            byte_offset: self.byte_offset,
            byte_length: self.byte_length,
        })
    }

    /// Validates that `end_column` is not less than `start_column` when both
    /// are present and the region occupies a single line.
    fn validate_same_line_columns(&self) -> crate::error::Result<()> {
        if let (Some(sc), Some(ec)) = (self.start_column, self.end_column)
            && ec < sc
        {
            return Err(SarifError::InvalidRegion(format!(
                "end_column ({ec}) must be >= start_column ({sc}) on the same line"
            )));
        }
        Ok(())
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
///     .with_region(RegionBuilder::new(10).with_end_line(15).build().expect("valid region"))
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
    use crate::error::SarifError;

    #[test]
    fn region_builder_minimal() {
        match RegionBuilder::new(5).build() {
            Ok(region) => {
                assert_eq!(region.start_line, 5);
                assert!(region.start_column.is_none());
                assert!(region.end_line.is_none());
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn region_builder_full() {
        match RegionBuilder::new(1)
            .with_start_column(5)
            .with_end_line(10)
            .with_end_column(20)
            .with_byte_offset(100)
            .with_byte_length(200)
            .build()
        {
            Ok(region) => {
                assert_eq!(region.start_line, 1);
                assert_eq!(region.start_column, Some(5));
                assert_eq!(region.end_line, Some(10));
                assert_eq!(region.end_column, Some(20));
                assert_eq!(region.byte_offset, Some(100));
                assert_eq!(region.byte_length, Some(200));
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn location_builder_minimal() {
        let loc = LocationBuilder::new("src/main.rs").build();
        assert_eq!(loc.physical_location.artifact_location.uri, "src/main.rs");
        assert!(loc.physical_location.region.is_none());
    }

    #[test]
    fn location_builder_with_region() {
        let region = match RegionBuilder::new(42).build() {
            Ok(r) => r,
            Err(e) => panic!("unexpected error: {e}"),
        };
        let loc = LocationBuilder::new("src/lib.rs")
            .with_region(region)
            .build();
        match loc.physical_location.region.as_ref() {
            Some(region) => assert_eq!(region.start_line, 42),
            None => panic!("expected region to be present"),
        }
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

    #[test]
    fn region_rejects_zero_start_line() {
        assert!(matches!(
            RegionBuilder::new(0).build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("start_line must be >= 1")
        ));
    }

    #[test]
    fn region_rejects_inverted_lines() {
        assert!(matches!(
            RegionBuilder::new(10).with_end_line(5).build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("end_line")
        ));
    }

    #[test]
    fn region_rejects_inverted_columns_on_same_line() {
        assert!(matches!(
            RegionBuilder::new(10)
                .with_end_line(10)
                .with_start_column(20)
                .with_end_column(5)
                .build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("end_column")
        ));
    }

    #[test]
    fn region_rejects_zero_start_column() {
        assert!(matches!(
            RegionBuilder::new(1).with_start_column(0).build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("start_column")
        ));
    }

    #[test]
    fn region_rejects_zero_end_column() {
        assert!(matches!(
            RegionBuilder::new(1).with_end_column(0).build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("end_column")
        ));
    }

    #[test]
    fn region_rejects_inverted_columns_implicit_single_line() {
        assert!(matches!(
            RegionBuilder::new(10)
                .with_start_column(20)
                .with_end_column(5)
                .build(),
            Err(SarifError::InvalidRegion(msg)) if msg.contains("end_column")
        ));
    }
}
