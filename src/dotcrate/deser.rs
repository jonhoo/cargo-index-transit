use serde::{de, Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(untagged, expecting = "expected a boolean or a string")]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

pub(super) fn version_trim_whitespace<'de, D>(deserializer: D) -> Result<semver::Version, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = semver::Version;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("SemVer version")
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match string.trim().parse().map_err(de::Error::custom) {
                Ok(parsed) => Ok(parsed),
                Err(e) => Err(e),
            }
        }
        fn visit_borrowed_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match string.trim().parse().map_err(de::Error::custom) {
                Ok(parsed) => Ok(parsed),
                Err(e) => Err(e),
            }
        }
    }

    deserializer.deserialize_any(Visitor)
}
