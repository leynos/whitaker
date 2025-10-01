use dylint_linting::config_or_default;
use rustc_lint::LateContext;
use serde::de::DeserializeOwned;

#[must_use]
pub fn load_or_default<'tcx, T>(_cx: &LateContext<'tcx>, lint_name: &str) -> T
where
    T: DeserializeOwned + Default,
{
    config_or_default(lint_name)
}

#[must_use]
pub fn decode_json_or_default<T>(raw: Option<&str>) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned + Default,
{
    raw.map(serde_json::from_str)
        .unwrap_or_else(|| Ok(T::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde::Deserialize;

    #[derive(Debug, Default, Deserialize, PartialEq, Eq)]
    struct ExampleConfig {
        flag: bool,
        #[serde(default)]
        threshold: u8,
    }

    #[rstest]
    fn returns_default_when_missing() {
        let parsed = decode_json_or_default::<ExampleConfig>(None).expect("default");
        assert_eq!(parsed, ExampleConfig::default());
    }

    #[rstest]
    fn parses_valid_payload() {
        let raw = Some("{\"flag\":true,\"threshold\":2}");
        let parsed = decode_json_or_default::<ExampleConfig>(raw).expect("parse valid JSON");
        assert!(parsed.flag);
        assert_eq!(parsed.threshold, 2);
    }

    #[rstest]
    fn surfaces_invalid_payload() {
        let err = decode_json_or_default::<ExampleConfig>(Some("not-json"));
        assert!(err.is_err());
    }
}
