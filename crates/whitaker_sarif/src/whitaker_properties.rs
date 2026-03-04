//! Whitaker-specific SARIF properties extension.
//!
//! [`WhitakerProperties`] carries clone detection metadata that is attached to
//! each SARIF result via the `properties` field. The struct serializes under a
//! `"whitaker"` key in the JSON property bag, matching the schema in
//! `docs/whitaker-clone-detector-design.md` §SARIF schema and mapping.
//!
//! Use [`WhitakerPropertiesBuilder`] for fluent construction.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::SarifError;

/// Whitaker-specific metadata attached to a SARIF result's `properties` field.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::WhitakerPropertiesBuilder;
///
/// let props = WhitakerPropertiesBuilder::new("T1")
///     .with_k(25)
///     .with_window(16)
///     .with_jaccard(0.92)
///     .with_cosine(0.88)
///     .with_group_id(1)
///     .with_class_size(4)
///     .build();
///
/// assert_eq!(props.profile, "T1");
/// assert_eq!(props.k, 25);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitakerProperties {
    /// Similarity profile: `"T1"`, `"T2"`, or `"T3"`.
    pub profile: String,
    /// k-shingle size.
    pub k: usize,
    /// Winnowing window size.
    pub window: usize,
    /// Jaccard similarity score.
    pub jaccard: f64,
    /// Cosine similarity score.
    pub cosine: f64,
    /// Clone group identifier.
    pub group_id: usize,
    /// Number of fragments in the clone class.
    pub class_size: usize,
}

/// Wraps properties under a `"whitaker"` key for embedding in a SARIF
/// property bag.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{WhitakerPropertiesBuilder};
/// use serde_json::Value;
///
/// let props = WhitakerPropertiesBuilder::new("T2")
///     .with_k(25)
///     .build();
/// let value: Value = props.into();
/// assert!(value.get("whitaker").is_some());
/// ```
impl From<WhitakerProperties> for Value {
    fn from(props: WhitakerProperties) -> Self {
        // Safety: WhitakerProperties always serializes to a JSON object.
        let inner = serde_json::to_value(props).unwrap_or(Value::Null);
        serde_json::json!({ "whitaker": inner })
    }
}

/// Extracts [`WhitakerProperties`] from a SARIF property bag value.
///
/// # Errors
///
/// Returns [`SarifError::MissingField`] if the `"whitaker"` key is absent.
/// Returns [`SarifError::Serialization`] if deserialization fails.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{WhitakerProperties, WhitakerPropertiesBuilder};
/// use serde_json::Value;
///
/// let props = WhitakerPropertiesBuilder::new("T1")
///     .with_k(25)
///     .build();
/// let value: Value = props.clone().into();
/// let extracted = WhitakerProperties::try_from(&value).expect("extract");
/// assert_eq!(extracted, props);
/// ```
impl TryFrom<&Value> for WhitakerProperties {
    type Error = SarifError;

    fn try_from(value: &Value) -> crate::error::Result<Self> {
        let inner = value
            .get("whitaker")
            .ok_or_else(|| SarifError::MissingField("whitaker".into()))?;
        serde_json::from_value(inner.clone()).map_err(SarifError::from)
    }
}

/// Fluent builder for [`WhitakerProperties`].
///
/// # Examples
///
/// ```
/// use whitaker_sarif::WhitakerPropertiesBuilder;
///
/// let props = WhitakerPropertiesBuilder::new("T3")
///     .with_jaccard(0.85)
///     .with_cosine(0.90)
///     .build();
/// assert_eq!(props.profile, "T3");
/// ```
#[derive(Debug, Clone)]
pub struct WhitakerPropertiesBuilder {
    profile: String,
    k: usize,
    window: usize,
    jaccard: f64,
    cosine: f64,
    group_id: usize,
    class_size: usize,
}

impl WhitakerPropertiesBuilder {
    /// Creates a builder with the given similarity profile.
    #[must_use]
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            k: 0,
            window: 0,
            jaccard: 0.0,
            cosine: 0.0,
            group_id: 0,
            class_size: 0,
        }
    }

    /// Sets the k-shingle size.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Sets the winnowing window size.
    #[must_use]
    pub fn with_window(mut self, window: usize) -> Self {
        self.window = window;
        self
    }

    /// Sets the Jaccard similarity score.
    #[must_use]
    pub fn with_jaccard(mut self, jaccard: f64) -> Self {
        self.jaccard = jaccard;
        self
    }

    /// Sets the cosine similarity score.
    #[must_use]
    pub fn with_cosine(mut self, cosine: f64) -> Self {
        self.cosine = cosine;
        self
    }

    /// Sets the clone group identifier.
    #[must_use]
    pub fn with_group_id(mut self, group_id: usize) -> Self {
        self.group_id = group_id;
        self
    }

    /// Sets the clone class size.
    #[must_use]
    pub fn with_class_size(mut self, class_size: usize) -> Self {
        self.class_size = class_size;
        self
    }

    /// Consumes the builder and produces [`WhitakerProperties`].
    #[must_use]
    pub fn build(self) -> WhitakerProperties {
        WhitakerProperties {
            profile: self.profile,
            k: self.k,
            window: self.window,
            jaccard: self.jaccard,
            cosine: self.cosine,
            group_id: self.group_id,
            class_size: self.class_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_produces_correct_properties() {
        let props = WhitakerPropertiesBuilder::new("T1")
            .with_k(25)
            .with_window(16)
            .with_jaccard(0.92)
            .with_cosine(0.88)
            .with_group_id(174)
            .with_class_size(4)
            .build();
        assert_eq!(props.profile, "T1");
        assert_eq!(props.k, 25);
        assert_eq!(props.window, 16);
        assert_eq!(props.group_id, 174);
        assert_eq!(props.class_size, 4);
    }

    #[test]
    fn into_value_wraps_under_whitaker_key() {
        let props = WhitakerPropertiesBuilder::new("T2").build();
        let value: Value = props.into();
        assert!(value.get("whitaker").is_some());
    }

    #[test]
    fn try_from_value_extracts_properties() {
        let props = WhitakerPropertiesBuilder::new("T1").with_k(10).build();
        let value: Value = props.clone().into();
        let extracted = WhitakerProperties::try_from(&value).expect("extract");
        assert_eq!(extracted, props);
    }

    #[test]
    fn try_from_value_without_key_returns_error() {
        let value = serde_json::json!({"other": 42});
        let result = WhitakerProperties::try_from(&value);
        assert!(result.is_err());
    }

    #[test]
    fn round_trip_via_serde() {
        let props = WhitakerPropertiesBuilder::new("T3")
            .with_jaccard(0.85)
            .with_cosine(0.90)
            .build();
        let json = serde_json::to_string(&props).expect("serialize");
        let parsed: WhitakerProperties = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(props, parsed);
    }
}
