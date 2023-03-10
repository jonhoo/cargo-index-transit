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
    prop_oneof![Just(None), "[a-z][a-z0-9_-]*".prop_map(|s| Some(s))]
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
        name in "[a-z][a-z0-9_-]*",
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

    if let Some(b) = &dep.2.optional {
        write!(&mut s, r#", optional = {b}"#).unwrap();
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
        deps in arb_deps()
        ) {

        // println!("{:?}", deps);
        roundtrip(
            |p| {
                use std::fmt::Write;
                // Modify the workspace before packaging
                let mut ctoml = std::fs::read_to_string(p.join("Cargo.toml")).unwrap();
                for dep in &deps {
                    write!(&mut ctoml, "\n{}", dep_to_toml(dep)).unwrap();
                }
                std::fs::write(p.join("Cargo.toml"), ctoml).unwrap();
            },
            |_, _, index| {
                // Check the various transit structs
                let mut num_found = 0;
                'check: for final_dep in index.dependencies.iter() {
                    for input_dep in &deps {
                        if input_dep.0 == final_dep.name {
                            let id = &input_dep.2;
                            let fd = &final_dep;
                            assert_eq!(id.version, fd.requirements);
                            assert_eq!(id.optional.unwrap_or(false), fd.optional);
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
