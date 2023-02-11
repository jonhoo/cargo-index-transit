use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap};

/// Section in which this dependency was defined
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    /// Used at run time
    Normal,
    /// Used at build time, not available at run time
    Build,
    /// Not fetched and not used, except for when used direclty in a workspace
    Dev,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct CrateVersion<'a> {
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    // cargo has this as string
    #[serde(rename = "vers")]
    pub version: semver::Version,
    #[serde(borrow)]
    #[serde(rename = "deps")]
    pub dependencies: Vec<Dependency<'a>>,
    #[serde(borrow)]
    pub features: BTreeMap<Cow<'a, str>, Vec<Cow<'a, str>>>,
    #[serde(borrow)]
    pub authors: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    pub description: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub documentation: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub homepage: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub readme: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub readme_file: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub keywords: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    pub categories: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    pub license: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub license_file: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub repository: Option<Cow<'a, str>>,
    #[serde(borrow)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Cow<'a, str>>,

    #[serde(default)]
    badges: BTreeMap<String, String>,
}

impl<'a> CrateVersion<'a> {
    pub fn new<Name, Feature: Ord>(
        mut m: super::dotcrate::NormalizedManifest<Name, Feature>,
        (readme, readme_contents): (Option<Cow<'a, str>>, Option<Cow<'a, str>>),
        is_for: &'_ str,
    ) -> Self
    where
        Name: Into<Cow<'a, str>>,
        Feature: Into<Cow<'a, str>>,
    {
        // let (readme, readme_contents) = match m.package.readme {
        //     Some(StringOrBool::Bool(false)) => (None, None),
        //     Some(StringOrBool::Bool(true)) => todo!("read README.md"),
        //     Some(StringOrBool::String(path)) => todo!("read {path}"),
        //     None => todo!("depends on file contents in .crate"),
        // };

        let deps = m
            .take_dependencies()
            .map(|(name_in_toml, d, kind)| {
                let (explicit_name, name) = match (d.package, name_in_toml) {
                    (Some(p), n) => {
                        // explicit_name = { package = name }
                        (Some(n.into()), Cow::Owned(p))
                    }
                    (None, n) => {
                        // explicit_name = { }
                        (None, n.into())
                    }
                };
                let is_from = match d.registry_index {
                    Some(r) => {
                        // not (necessarily) from crates.io
                        Cow::Owned(r)
                    }
                    None => {
                        // from crates.io
                        Cow::Borrowed("https://github.com/rust-lang/crates.io-index")
                    }
                };
                let target_registry_dependent_src_registry = if is_from == is_for {
                    None
                } else {
                    Some(is_from)
                };
                Dependency {
                    optional: d.optional.unwrap_or(false),
                    default_features: d.default_features.unwrap_or(true),
                    name,
                    features: d
                        .features
                        .unwrap_or_default()
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    requirements: d.version,
                    target: d.target.map(Into::into),
                    kind,
                    registry: target_registry_dependent_src_registry,
                    explicit_name_in_toml: explicit_name,
                }
            })
            .collect();

        Self {
            name: m.package.name.into(),
            version: m.package.version,
            dependencies: deps,
            features: m
                .features
                .unwrap_or_default()
                .into_iter()
                .map(|(k, vs)| (k.into(), vs.into_iter().map(Into::into).collect()))
                .collect(),
            authors: m
                .package
                .authors
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            description: m.package.description.map(Into::into),
            documentation: m.package.documentation.map(Into::into),
            homepage: m.package.homepage.map(Into::into),
            readme: readme_contents,
            readme_file: readme,
            keywords: m
                .package
                .keywords
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            categories: m
                .package
                .categories
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            license: m.package.license.map(Into::into),
            license_file: m.package.license_file.map(Into::into),
            repository: m.package.repository.map(Into::into),
            links: m.package.links.map(Into::into),
            badges: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Dependency<'a> {
    pub optional: bool,
    pub default_features: bool,
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    #[serde(borrow)]
    pub features: Vec<Cow<'a, str>>,
    // cargo and crates-io have this as string
    #[serde(rename = "version_req")]
    pub requirements: semver::VersionReq,
    #[serde(borrow)]
    pub target: Option<Cow<'a, str>>,
    // crates-io has this as option
    pub kind: DependencyKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub registry: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub explicit_name_in_toml: Option<Cow<'a, str>>,
}
