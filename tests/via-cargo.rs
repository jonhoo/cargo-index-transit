use cargo::core::Shell;
use cargo::ops::NewProjectKind;
use cargo_index_transit as cit;
use flate2::read::GzDecoder;
use std::borrow::Cow;
use std::io::Read;
use std::path::Path;

fn roundtrip(
    setup: impl FnOnce(&Path),
    check: impl FnOnce(
        &cit::dotcrate::NormalizedManifest<String, String>,
        &cit::publish::CrateVersion<'_>,
        &cit::index::Entry<
            Cow<'_, str>,
            semver::Version,
            semver::VersionReq,
            Cow<'_, str>,
            Cow<'_, str>,
            Cow<'_, str>,
        >,
    ),
) {
    let d = tempfile::tempdir().unwrap();
    let mut config = cargo::Config::new(
        Shell::default(),
        d.path().to_path_buf(),
        d.path().join("cargo-home"),
    );
    config
        .configure(0, false, None, false, false, false, &None, &[], &[])
        .unwrap();

    let package = d.path().join("roundtrip");

    cargo::ops::new(
        &cargo::ops::NewOptions {
            version_control: None,
            kind: NewProjectKind::Lib,
            auto_detect_kind: false,
            path: package.clone(),
            name: None,
            edition: None,
            registry: None,
        },
        &config,
    )
    .unwrap();

    setup(&package);

    let ws = cargo::core::Workspace::new(&package.join("Cargo.toml"), &config).unwrap();

    let tarball = cargo::ops::package_one(
        &ws,
        ws.current().unwrap(),
        &cargo::ops::PackageOpts {
            config: &config,
            list: false,
            check_metadata: false,
            allow_dirty: true,
            verify: false,
            jobs: None,
            keep_going: false,
            to_package: cargo::ops::Packages::Default,
            targets: Vec::new(),
            cli_features: cargo::core::resolver::CliFeatures::new_all(false),
        },
    )
    .unwrap()
    .unwrap();

    let decoder = GzDecoder::new(tarball.file());
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();

        if path.ends_with("Cargo.toml") {
            let mut manifest = String::new();
            entry.read_to_string(&mut manifest).unwrap();

            let m: cit::dotcrate::NormalizedManifest<String, String> =
                toml_edit::de::from_str(&manifest).unwrap();

            let p: cit::publish::CrateVersion<'_> = cit::publish::CrateVersion::new(
                m.clone(),
                (None, None),
                "https://github.com/rust-lang/crates.io-index",
            );
            let json = serde_json::to_string(&p).unwrap();
            let p2: crates_io::NewCrate = serde_json::from_str(&json).unwrap();
            let json = serde_json::to_string(&p2).unwrap();
            let p3: cit::publish::CrateVersion<'_> = serde_json::from_str(&json).unwrap();
            assert_eq!(p, p3);

            let i = cit::index::Entry::from_publish(p.clone(), [0; 32]);
            let json = serde_json::to_string(&i).unwrap();
            let _: cargo::sources::registry::RegistryPackage = serde_json::from_str(&json).unwrap();
            let i2: crates_index::Version = serde_json::from_str(&json).unwrap();
            let json = serde_json::to_string(&i2).unwrap();
            let i3: cit::index::Entry<_, _, _, _, _, _> = serde_json::from_str(&json).unwrap();
            assert_eq!(i, i3);

            assert_eq!(i.name, "roundtrip");
            assert_eq!(i.version, semver::Version::new(0, 1, 0));

            check(&m, &p, &i);

            break;
        }
    }
}

#[test]
fn roundtrip_simplest() {
    roundtrip(|_| {}, |_, _, _| {});
}

#[test]
fn roundtrip_one_dep() {
    roundtrip(
        |_p| {
            // TODO: add a dependency to p/Cargo.toml
        },
        |_, _, _index| {
            // TODO: check that index entry contains appropriate dependency specifier
        },
    );
}
