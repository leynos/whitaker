use std::convert::Infallible;
use std::str::FromStr;

/// Wrapper for locale values supplied via behaviour-driven test steps.
#[derive(Clone, Debug)]
pub struct StepLocale {
    raw: String,
}

impl FromStr for StepLocale {
    type Err = Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let raw = input
            .trim()
            .trim_matches(|candidate| matches!(candidate, '"' | '\''))
            .to_owned();

        Ok(Self { raw })
    }
}

impl StepLocale {
    /// Consumes the step value, yielding the parsed string.
    pub fn into_inner(self) -> String {
        self.raw
    }
}
