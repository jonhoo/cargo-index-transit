use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

/// A single line in the index representing a single version of a package.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry<Name, Version, Req, Feature, Target, Links>
where
    Feature: Ord,
{
    pub name: Name,
    #[serde(rename = "vers")]
    pub version: Version,

    // These are Arc so that they can be deduplicated easily by calling code if they happen to be
    // reading it all of the versions of a single crate at once (as nearby versions often share
    // dependency and feature lists).
    #[serde(rename = "deps")]
    pub dependencies: Arc<[RegistryDependency<Name, Req, Feature, Target>]>,

    pub features: Arc<BTreeMap<Feature, Vec<Feature>>>,

    /// This field contains features with new, extended syntax. Specifically,
    /// namespaced features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// This is separated from `features` because versions older than 1.19
    /// will fail to load due to not being able to parse the new syntax, even
    /// with a `Cargo.lock` file.
    ///
    /// It's wrapped in a `Box` to reduce size of the struct when the field is unused (i.e. almost
    /// always).
    /// <https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html#index-changes>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features2: Option<Box<BTreeMap<Feature, Vec<Feature>>>>,

    #[serde(with = "hex")]
    #[serde(rename = "cksum")]
    pub checksum: [u8; 32],

    /// If `true`, Cargo will skip this version when resolving.
    #[serde(default)]
    pub yanked: bool,

    /// Native library name this package links to.
    ///
    /// Added early 2018 (see <https://github.com/rust-lang/cargo/pull/4978>),
    /// can be `None` if published before then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,

    /// The schema version for this entry.
    ///
    /// If this is None, it defaults to version 1. Entries with unknown
    /// versions are ignored.
    ///
    /// Version `2` format adds the `features2` field.
    ///
    /// This provides a method to safely introduce changes to index entries
    /// and allow older versions of cargo to ignore newer entries it doesn't
    /// understand. This is honored as of 1.51, so unfortunately older
    /// versions will ignore it, and potentially misinterpret version 2 and
    /// newer entries.
    ///
    /// The intent is that versions older than 1.51 will work with a
    /// pre-existing `Cargo.lock`, but they may not correctly process `cargo
    /// update` or build a lock from scratch. In that case, cargo may
    /// incorrectly select a new package that uses a new index format. A
    /// workaround is to downgrade any packages that are incompatible with the
    /// `--precise` flag of `cargo update`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "v")]
    pub schema_version: Option<u8>,
}

impl<'a>
    Entry<
        Cow<'a, str>,
        semver::Version,
        semver::VersionReq,
        Cow<'a, str>,
        Cow<'a, str>,
        Cow<'a, str>,
    >
{
    pub fn from_manifest<Name, Feature: Ord>(
        v: super::dotcrate::NormalizedManifest<Name, Feature>,
        via_registry: &'_ str,
        checksum: [u8; 32],
    ) -> Self
    where
        Name: Into<Cow<'a, str>>,
        Feature: Into<Cow<'a, str>>,
    {
        let in_registry = super::publish::CrateVersion::new(v, (None, None), via_registry);
        Self::from_publish(in_registry, checksum)
    }

    pub fn from_publish(v: super::publish::CrateVersion<'a>, checksum: [u8; 32]) -> Self {
        let (features, features2): (BTreeMap<_, _>, BTreeMap<_, _>) =
            v.features.into_iter().partition(|(_k, vals)| {
                !vals
                    .iter()
                    .any(|v| v.starts_with("dep:") || v.contains("?/"))
            });
        let (features2, schema_version) = if features2.is_empty() {
            (None, None)
        } else {
            (Some(features2), Some(2))
        };

        Self {
            name: v.name,
            version: v.version,
            dependencies: Arc::from(
                v.dependencies
                    .into_iter()
                    .map(|d| {
                        let (name, package) = match (d.name, d.explicit_name_in_toml) {
                            (p, Some(n)) => (n, Some(Box::new(p))),
                            (n, None) => (n, None),
                        };
                        RegistryDependency {
                            name,
                            kind: Some(d.kind),
                            requirements: d.requirements,
                            features: Box::new(d.features.into_boxed_slice()),
                            optional: d.optional,
                            default_features: d.default_features,
                            target: d.target.map(Box::new),
                            registry: d
                                .registry
                                .map(|r| r.into_owned().into_boxed_str())
                                .map(Box::new),
                            package,
                            public: None,
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            features: Arc::new(features),
            features2: features2.map(Box::new),
            checksum,
            yanked: false,
            links: v.links.map(Into::into),
            schema_version,
        }
    }
}

/// A dependency as encoded in the index JSON.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegistryDependency<Name, Req, Feature, Target> {
    // In old `cargo` versions the dependency order appears to matter if the same dependency exists
    // twice but with different `kind` fields. In those cases the `optional` field can sometimes be
    // ignored or misinterpreted. By placing the fields in this order, we ensure that `normal`
    // dependencies are always first when multiple with the same `name` exist.
    pub name: Name,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<super::publish::DependencyKind>,

    #[serde(rename = "req")]
    pub requirements: Req,

    pub features: Box<Box<[Feature]>>,
    pub optional: bool,
    pub default_features: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Box<Target>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Box<Box<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<Box<Name>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}
