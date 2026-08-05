//! `CoreMaintenance.maintenanceUiFromCheck` parses the `update check` stdout
//! document in QML; `UpdateCheckDocument` is its Rust emitter. The repo bans
//! every JS runtime, so a Rust test cannot call the QML function — instead
//! both sides are pinned to the shared fixtures in
//! `tests/fixtures/update-check/`, which must round-trip byte-exactly through
//! the real serializer. `tests/qml/tst_Maintenance.qml` feeds the same bytes
//! to the QML parser, so either side drifting fails its own suite.
//!
//! This seam exists because the previous QML parser read a key set the helper
//! never emitted (`updateAvailable`, `currentVersion`, `targetVersion`,
//! `releaseNotesUrl` at top level) and its tests fed that invented format
//! back to it — green tests, broken product.
//!
//! BUNDLE-021 v-next (git-plugin-distribution Task 1): the document now
//! carries `reinstallRequired` and no archive/checksum/source-commit fields
//! at all — discovery reads the dist repo's `bundle.json` receipt instead of
//! GitHub Releases. QML's `reinstallRequired` handling is a later task; this
//! file only pins the Rust-side shape and fixture contents for now.

use agent_bar::plugin::maintenance::UpdateCheckDocument;

fn fixture_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update-check")
}

fn fixture(name: &str) -> UpdateCheckDocument {
    let raw = std::fs::read(fixture_dir().join(name)).expect("read fixture");
    UpdateCheckDocument::parse_json(&raw).expect("fixture must satisfy the real validator")
}

/// Every fixture is the exact stdout the helper writes: it passes the
/// production validator and serializes back to the identical bytes.
#[test]
fn fixtures_are_exact_update_check_stdout() {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(fixture_dir()).expect("read fixture dir") {
        let path = entry.expect("dir entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("fixture name")
            .to_string();
        let raw = std::fs::read(&path).expect("read fixture");
        let doc = UpdateCheckDocument::parse_json(&raw)
            .unwrap_or_else(|e| panic!("{name} must satisfy the real validator: {e}"));
        let round_trip = doc.to_stdout_json().expect("serialize fixture");
        assert_eq!(
            round_trip.as_bytes(),
            raw.as_slice(),
            "{name} must be byte-exact `update check` stdout"
        );
        names.push(name);
    }
    for required in [
        "available.json",
        "no-compatible.json",
        "up-to-date.json",
        "reinstall-required.json",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing required fixture {required}"
        );
    }
}

/// The four fixtures cover every answer the document can give the UI.
#[test]
fn fixture_semantics_cover_every_answer() {
    let available = fixture("available.json");
    assert!(available.available);
    assert!(!available.reinstall_required);
    let latest = available
        .latest_compatible
        .expect("available fixture names a target");
    assert_ne!(latest.version, available.current.version);
    assert!(!latest.release_notes_url.is_empty());

    let up_to_date = fixture("up-to-date.json");
    assert!(!up_to_date.available);
    assert!(!up_to_date.reinstall_required);
    let same = up_to_date
        .latest_compatible
        .expect("up-to-date still describes the newest compatible release");
    assert_eq!(same.version, up_to_date.current.version);

    let none = fixture("no-compatible.json");
    assert!(!none.available);
    assert!(!none.reinstall_required);
    assert!(none.latest_compatible.is_none());

    let reinstall = fixture("reinstall-required.json");
    assert!(reinstall.reinstall_required);
    assert!(!reinstall.available);
    assert!(reinstall.latest_compatible.is_none());
}

/// The rewritten document (BUNDLE-021 v-next) carries no archive-download
/// fields anywhere: discovery is a dist repo receipt, not a GitHub release
/// asset list, and `update apply` no longer downloads/verifies an archive.
#[test]
fn fixtures_carry_no_archive_fields() {
    for entry in std::fs::read_dir(fixture_dir()).expect("read fixture dir") {
        let path = entry.expect("dir entry").path();
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        for forbidden in ["archiveUrl", "checksumUrl", "archiveSha256", "sourceCommit"] {
            assert!(
                !raw.contains(forbidden),
                "{} must not contain {forbidden}",
                path.display()
            );
        }
    }
}

/// The seam is only real while the QML side parses the real key set.
#[test]
fn qml_parser_reads_the_real_keys() {
    let js = std::fs::read_to_string("assets/omarchy/CoreMaintenance.js")
        .expect("read CoreMaintenance.js");
    assert!(
        js.contains("function maintenanceUiFromCheck("),
        "CoreMaintenance.maintenanceUiFromCheck is the other half of this seam"
    );
    assert!(
        js.contains("latestCompatible"),
        "the parser must read the BUNDLE-021 document"
    );
    assert!(
        !js.contains("updateAvailable"),
        "the invented top-level key set must not come back"
    );
}
