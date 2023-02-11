use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod deser;
pub use deser::StringOrBool;
use deser::*;

/// A `Cargo.toml` manifest from or for a `.crate` file.
// NOTE: This doesn't use borrowing deserialization because toml_edit doesn't support it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NormalizedManifest<Name, Feature>
where
    Feature: Ord,
{
    pub package: Package<Name>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<BTreeMap<String, Dependency<Feature>>>,
    #[serde(alias = "dev_dependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_dependencies: Option<BTreeMap<String, Dependency<Feature>>>,
    #[serde(alias = "build_dependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_dependencies: Option<BTreeMap<String, Dependency<Feature>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<BTreeMap<Feature, Vec<Feature>>>,
}

impl<Name, Feature> NormalizedManifest<Name, Feature>
where
    Feature: Ord,
{
    pub(crate) fn take_dependencies(
        &mut self,
    ) -> impl Iterator<Item = (String, Dependency<Feature>, super::publish::DependencyKind)> {
        self.dependencies
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(|d| (d, super::publish::DependencyKind::Normal))
            .chain(
                self.dev_dependencies
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|d| (d, super::publish::DependencyKind::Dev)),
            )
            .chain(
                self.build_dependencies
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|d| (d, super::publish::DependencyKind::Build)),
            )
            .map(|((name_in_toml, d), kind)| (name_in_toml, d, kind))
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Dependency<Feature> {
    pub version: semver::VersionReq,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<Feature>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(alias = "default_features")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    /// A platform name, like `x86_64-apple-darwin`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Represents the `package`/`project` sections of a `Cargo.toml`.
///
/// Note that the order of the fields matters, since this is the order they
/// are serialized to a TOML file. For example, you cannot have values after
/// the field `metadata`, since it is a table and values cannot appear after
/// tables.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Package<Name> {
    pub rust_version: Option<String>,
    pub name: Name,
    #[serde(deserialize_with = "version_trim_whitespace")]
    pub version: semver::Version,
    pub links: Option<String>,

    // Package metadata.
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub readme: Option<StringOrBool>,
    pub keywords: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
}
