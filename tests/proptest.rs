use std::collections::HashSet;

use cargo_index_transit::{dotcrate, publish::DependencyKind};
use proptest::prelude::*;

mod util;
use util::roundtrip;

#[derive(Debug)]
struct Dependency(String, DependencyKind, dotcrate::Dependency<String>);

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
    prop_oneof![Just(None), "[a-z][a-z0-9_-]{0,3}".prop_map(|s| Some(s))]
}

prop_compose! {
    fn arb_dep_listing()(
        package in arb_package(),
        version in "([=^<>]|<=|>=)?(0|[1-9][0-9]{0,1})(\\.(0|[1-9][0-9]{0,2})){0,2}",
        optional in any::<Option<bool>>(),
        default_features in any::<Option<bool>>(),
    ) -> dotcrate::Dependency<String> {
        let req = semver::VersionReq::parse(&version);
        let req = req.unwrap();

        // TODO: features, incl. dep: and pkg?/ features
        // ref https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html#new-syntax-for-cargo-features

        dotcrate::Dependency {
            version: req,
            registry_index: None,
            features: None,
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
        name in "[a-z][a-z0-9_-]{0,3}",
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

    s.push('}');

    s
}

fn arb_deps() -> impl Strategy<Value = Vec<Dependency>> {
    prop::collection::vec(arb_dep(), 1..5)
}

proptest! {
    #[test]
    #[ignore = "proptests are slow and should be run explicitly"]
    fn roundtrip_one_dep(
        mut deps in arb_deps()
        ) {

        // Ignore duplicate entries.
        // Ideally we'd express this in the Strategy, but doing so is quite tricky
        {
            let mut names = HashSet::new();
            deps.retain(|Dependency(name, _, _)| names.insert(name.to_string()));
        }

        // println!("{:?}", deps);
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
                            num_found += 1;
                            continue 'check;
                        }
                    }
                    panic!();
                }
                assert_eq!(num_found, deps.len());
            },
        );
    }
}
