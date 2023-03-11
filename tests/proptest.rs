use std::{borrow::Cow, collections::HashSet, sync::Arc};

use cargo_index_transit::{dotcrate, publish::DependencyKind};
use proptest::prelude::*;

mod util;
use util::roundtrip;

#[derive(Debug, Clone)]
struct Dependency(String, DependencyKind, dotcrate::Dependency<String>);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum FeatureSpec {
    Feature(String),
    Dep(String),
    Strong(String, String),
    Weak(String, String),
}

prop_compose! {
    fn arb_dep_kind()(kind in 0u8..3) -> DependencyKind {
        match kind {
            0 => DependencyKind::Normal,
            1 => DependencyKind::Build,
            2 => DependencyKind::Dev,
            _ => panic!(),
        }
    }
}

fn arb_package() -> impl Strategy<Value = Option<String>> {
    prop_oneof![Just(None), "[a-z][a-z0-9_-]{0,2}".prop_map(|s| Some(s))]
}

fn arb_feature_name() -> impl Strategy<Value = String> + Copy {
    "[a-z][a-z0-9_-]?"
}

fn arb_maybe_dep_feature() -> impl Strategy<Value = Option<(String, bool)>> {
    prop_oneof![
        Just(None),
        (arb_feature_name(), any::<bool>()).prop_map(|s| Some(s))
    ]
}

fn arb_dep_feature(deps: Arc<[Dependency]>) -> impl Strategy<Value = FeatureSpec> {
    let ndeps = deps.len();
    // Or it can reference a dependency or a feature of a dependency
    assert_ne!(ndeps, 0);
    (0..ndeps, arb_maybe_dep_feature()).prop_map(move |(i, ft)| match ft {
        None => FeatureSpec::Dep(deps[i].0.clone()),
        Some((ft, weak)) => {
            if weak {
                FeatureSpec::Weak(deps[i].0.clone(), ft)
            } else {
                FeatureSpec::Strong(deps[i].0.clone(), ft)
            }
        }
    })
}

fn arb_feature_feature(
    base_features: Arc<[String]>,
    not: usize,
) -> impl Strategy<Value = FeatureSpec> {
    // A feature spec can reference another feature
    assert_ne!(base_features.len() - 1, 0);
    (0..(base_features.len() - 1)).prop_map(move |mut i| {
        if i >= not {
            i += 1;
        }
        FeatureSpec::Feature(base_features[i].clone())
    })
}

fn arb_dep_features() -> impl Strategy<Value = Option<Vec<String>>> {
    prop_oneof![
        Just(None),
        prop::collection::vec(arb_feature_name(), 0..3).prop_map(Some)
    ]
}

prop_compose! {
    fn arb_dep_listing()(
        package in arb_package(),
        version in "([=^<>])?(0|[1-9][0-9]{0,1})(\\.(0|[1-9][0-9]{0,2})){0,2}",
        optional in any::<Option<bool>>(),
        default_features in any::<Option<bool>>(),
        features in arb_dep_features()
    ) -> dotcrate::Dependency<String> {
        let req = semver::VersionReq::parse(&version);
        let req = req.unwrap();

        dotcrate::Dependency {
            version: req,
            registry_index: None,
            features,
            optional,
            public: None,
            default_features,
            package,
            target: None,
        }
    }
}

prop_compose! {
    fn arb_dep()(
        name in "[a-z][a-z0-9_-]{0,2}",
        kind in arb_dep_kind(),
        listing in arb_dep_listing()
    ) -> Dependency {
        Dependency(name, kind, listing)
    }
}

fn dep_to_toml(dep: &Dependency) -> String {
    use std::fmt::Write;

    let mut s = format!(r#""{}" = {{"#, dep.0);
    write!(&mut s, r#"version = "{}""#, dep.2.version).unwrap();

    // optional is only permitted for normal dependencies
    if matches!(dep.1, DependencyKind::Normal) {
        if let Some(b) = &dep.2.optional {
            write!(&mut s, r#", optional = {b}"#).unwrap();
        }
    }
    if let Some(b) = &dep.2.default_features {
        write!(&mut s, r#", default-features = {b}"#).unwrap();
    }
    if let Some(p) = &dep.2.package {
        write!(&mut s, r#", package = "{p}""#).unwrap();
    }
    if let Some(fs) = &dep.2.features {
        write!(&mut s, r#", features = ["#).unwrap();
        for fi in 0..fs.len() {
            if fi != 0 {
                write!(&mut s, r#","#).unwrap();
            }
            write!(&mut s, r#""{}""#, fs[fi]).unwrap();
        }
        write!(&mut s, r#"]"#).unwrap();
    }

    s.push('}');

    s
}

fn arb_deps() -> impl Strategy<Value = Vec<Dependency>> {
    prop::collection::vec(arb_dep(), 1..4).prop_map(|mut deps| {
        // Ignore duplicate entries.
        // Ideally we'd express this in the Strategy, but doing so is quite tricky
        {
            let mut names = HashSet::new();
            deps.retain(|Dependency(name, _, _)| names.insert(name.to_string()));
        }
        deps
    })
}

fn arb_base_features() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(arb_feature_name(), 0..3).prop_map(|mut deps| {
        // Ignore duplicate features.
        // Ideally we'd express this in the Strategy, but doing so is quite tricky
        {
            let mut names = HashSet::new();
            deps.retain(|name| names.insert(name.to_string()));
        }
        deps
    })
}

fn arb_feature_specs(
    deps: Vec<Dependency>,
    base_features: Vec<String>,
) -> impl Strategy<Value = Vec<(String, Vec<FeatureSpec>)>> {
    let optional_deps: Arc<[_]> = Arc::from(
        deps.into_iter()
            .filter(|dep| matches!(dep.1, DependencyKind::Normal) && dep.2.optional == Some(true))
            .collect::<Box<[_]>>(),
    );
    let base_features: Arc<[_]> = Arc::from(base_features.into_boxed_slice());
    (0..base_features.len())
        .map(move |i| {
            let spec = match (optional_deps.len(), base_features.len()) {
                (0, 0 | 1) => Just(Vec::new()).boxed(),
                (0, n) => prop::collection::vec(
                    arb_feature_feature(Arc::clone(&base_features), i),
                    0..(n - 1),
                )
                .boxed(),
                (_, 0 | 1) => {
                    prop::collection::vec(arb_dep_feature(Arc::clone(&optional_deps)), 0..2).boxed()
                }
                _ => prop::collection::vec(
                    prop_oneof![
                        arb_feature_feature(Arc::clone(&base_features), i),
                        arb_dep_feature(Arc::clone(&optional_deps))
                    ],
                    0..2,
                )
                .boxed(),
            };
            (
                Just(base_features[i].clone()),
                spec.prop_map(|fs| {
                    // Avoid duplicate feature dependencies
                    fs.into_iter()
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                }),
            )
        })
        .collect::<Vec<_>>()
}

prop_compose! {
    fn arb_spec()(
        deps in arb_deps(),
        base_features in arb_base_features(),
    )(
        specs in arb_feature_specs(deps.clone(), base_features),
        deps in Just(deps)
    ) -> (Vec<Dependency>, Vec<(String, Vec<FeatureSpec>)>) {
            (deps, specs)
    }
}

proptest! {
    // For the lib.rs/main.rs warning, see https://github.com/proptest-rs/proptest/issues/233
    // 512 here was determined based on CI time. 1024 took about 5m without coverage and 15m with.
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    #[ignore = "proptests are slow and should be run explicitly"]
    fn merry_go_round(
        (deps, mut features) in arb_spec(),
        ) {

        // Cargo requires that there is always a feature for every optional dep:
        // https://github.com/rust-lang/cargo/blob/7b2fabf785755458ca02a00140060d8ba786a3ff/src/cargo/core/summary.rs#L339-L349
        // Make that be the case.
        let unmentioned_optional = deps.iter()
            .filter(|dep| matches!(dep.1, DependencyKind::Normal) && dep.2.optional == Some(true))
            .filter(|Dependency(dep, _, _)| !features.iter().flat_map(|(_, specs)| specs).any(|spec| {
                match spec {
                    // NOTE: *technically* this may not count for cargo if the same feature uses
                    // dep: elsewhere, since then the plain feature specifiers are considered as
                    // being in a different namespace. But it seems to be working okay for now.
                    FeatureSpec::Feature(f) => f == dep,
                    FeatureSpec::Dep(d) => d == dep,
                    FeatureSpec::Weak(d, _) => d == dep,
                    FeatureSpec::Strong(d, _) => d == dep,
                }
            }));
        let add: Vec<_> = unmentioned_optional.map(|Dependency(dep, _, _)| FeatureSpec::Dep(dep.clone())).collect();
        if !add.is_empty() {
            features.push(("fix-optional".into(), add));
        }

        roundtrip(
            |p| {
                use std::fmt::Write;
                // Modify the workspace before packaging
                let mut ctoml = std::fs::read_to_string(p.join("Cargo.toml")).unwrap();
                // There's already a [dependencies] at the bottom of a fresh Cargo.toml
                for dep in deps.iter().filter(|&Dependency(_, kind, _)| matches!(kind, DependencyKind::Normal)) {
                    write!(&mut ctoml, "\n{}", dep_to_toml(dep)).unwrap();
                }
                write!(&mut ctoml, "\n[dev-dependencies]").unwrap();
                for dep in deps.iter().filter(|&Dependency(_, kind, _)| matches!(kind, DependencyKind::Dev)) {
                    write!(&mut ctoml, "\n{}", dep_to_toml(dep)).unwrap();
                }
                write!(&mut ctoml, "\n[build-dependencies]").unwrap();
                for dep in deps.iter().filter(|&Dependency(_, kind, _)| matches!(kind, DependencyKind::Build)) {
                    write!(&mut ctoml, "\n{}", dep_to_toml(dep)).unwrap();
                }
                // Write out the feature list
                write!(&mut ctoml, "\n[features]").unwrap();
                for (f, fdeps) in &features {
                    write!(&mut ctoml, "\n{f} = [").unwrap();
                    for fdi in 0..fdeps.len() {
                        if fdi != 0 {
                            write!(&mut ctoml, ",").unwrap();
                        }
                        match &fdeps[fdi] {
                            FeatureSpec::Feature(f) => write!(&mut ctoml, r#""{f}""#).unwrap(),
                            FeatureSpec::Dep(f) => write!(&mut ctoml, r#""dep:{f}""#).unwrap(),
                            FeatureSpec::Strong(d, f) => write!(&mut ctoml, r#""{d}/{f}""#).unwrap(),
                            FeatureSpec::Weak(d, f) => write!(&mut ctoml, r#""{d}?/{f}""#).unwrap(),

                        }
                    }
                    write!(&mut ctoml, "]").unwrap();
                }
                // eprintln!("{ctoml}");
                std::fs::write(p.join("Cargo.toml"), ctoml).unwrap();
            },
            |_, _, index| {
                // Check the various transit structs
                let mut num_found = 0;
                'check: for final_dep in index.dependencies.iter() {
                    for Dependency(iname, ikind, id) in &deps {
                        if iname == &*final_dep.name {
                            let fd = &final_dep;
                            assert_eq!(Some(ikind), fd.kind.as_ref());
                            assert_eq!(id.version, fd.requirements);
                            if matches!(ikind, DependencyKind::Normal) {
                                assert_eq!(id.optional.unwrap_or(false), fd.optional);
                            } else {
                                assert!(!fd.optional);
                            }
                            assert_eq!(id.default_features.unwrap_or(true), fd.default_features);
                            assert_eq!(id.package.as_deref(), fd.package.as_ref().map(|s| &***s));
                            assert_eq!(id.features.as_deref().unwrap_or(&[]), &fd.features[..]);
                            num_found += 1;
                            continue 'check;
                        }
                    }
                    panic!();
                }
                assert_eq!(num_found, deps.len());

                for (fname, inspecs) in &features {
                    let features = if inspecs.iter().all(|spec| matches!(spec, FeatureSpec::Feature(_) | FeatureSpec::Strong(_, _))) {
                        &*index.features
                    } else {
                        index.features2.as_deref().expect("feature should be there, so map shouldn't be empty")
                    };
                    let outspecs = &features[&**fname];
                    for inspec in inspecs {
                        match inspec {
                            FeatureSpec::Feature(f) => {
                                assert!(outspecs.contains(&Cow::Borrowed(f)));
                            }
                            FeatureSpec::Dep(d) => {
                                assert!(outspecs.iter().any(|spec| spec.strip_prefix("dep:") == Some(d)));
                            }
                            FeatureSpec::Strong(d, f) => {
                                assert!(outspecs.iter().any(|spec| {
                                    spec.split_once("/") == Some((d, f))
                                }));
                            }
                            FeatureSpec::Weak(d, f) => {
                                assert!(outspecs.iter().any(|spec| {
                                    spec.split_once("?/") == Some((d, f))
                                }));
                            }
                        }
                    }
                }
            },
        );
    }
}
