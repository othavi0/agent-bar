//! Root-tree completeness (monorepo migration Task 2).
//!
//! The plugin QML/JS/manifest tree now lives at the repo root alongside
//! `src/`, `docs/`, and `target/`. `BundleBuilder::stamp` reads that root,
//! stamps in the private helper and marketplace preview image, and writes
//! `bundle.json` scoped to `SHIPPED_ROOT_FILES`/`SHIPPED_DIRS` only.
//! `BundleValidator::validate_tree` covers receipt/filesystem consistency but
//! never learned the shell's own manifest grammar (id regex, entry point
//! existence, `kinds`, `defaultSection`), so this test reimplements that
//! grammar directly in Rust -- mirroring `omarchy-plugin-validate` -- rather
//! than shelling out to it, so the gate still runs in a CI container that has
//! never heard of Omarchy.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use agent_bar::plugin::bundle::{BundleBuilder, BundleValidator};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to);
        } else if ty.is_file() {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// Shipped top-level files that live directly at the real repo root, copied
/// verbatim into the fake fixture root. `manifest.json` is included so the
/// fixture carries the real id/kinds/entryPoints grammar this test checks.
const ROOT_SOURCE_FILES: &[&str] = &[
    "BarWidget.qml",
    "CoreMaintenance.js",
    "CoreScroll.js",
    "CoreService.js",
    "CoreSettings.js",
    "CoreView.js",
    "MaintenanceView.qml",
    "Popup.qml",
    "ProviderRail.qml",
    "ProviderView.qml",
    "Service.qml",
    "SettingsView.qml",
    "manifest.json",
];

/// Build a throwaway "source repo" containing everything `stamp` reads.
///
/// The QML/JS/manifest/`components`/`icons` tree is the real one, so the
/// manifest and entry points this test checks are the ones that actually
/// ship. The terminal helper, README, LICENSE, and demo image are small
/// fakes: their exact bytes are not under test here, only that `stamp`
/// picks them up and the resulting root is contract-complete. Non-shipped
/// noise (`src/`, `docs/dev/`, `Cargo.toml`) stands in for the rest of the
/// monorepo that `stamp` must tolerate and ignore.
fn fake_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    for name in ROOT_SOURCE_FILES {
        fs::copy(workspace_root().join(name), root.join(name)).unwrap();
    }
    copy_dir_all(
        &workspace_root().join("components"),
        &root.join("components"),
    );
    copy_dir_all(&workspace_root().join("icons"), &root.join("icons"));

    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/agent-bar-open-terminal"),
        b"#!/bin/bash\nexit 0\n",
    )
    .unwrap();

    fs::write(root.join("LICENSE"), b"Fake license text.\n").unwrap();
    fs::write(root.join("README.md"), b"# Fake readme\n").unwrap();

    fs::create_dir_all(root.join("docs/media")).unwrap();
    fs::write(
        root.join("docs/media/demo.png"),
        b"not a real png, just stand-in bytes",
    )
    .unwrap();

    // Non-shipped noise that must be tolerated and excluded from the receipt.
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), b"// noise, not shipped\n").unwrap();
    fs::create_dir_all(root.join("docs/dev")).unwrap();
    fs::write(root.join("docs/dev/notes.md"), b"noise, not shipped\n").unwrap();
    fs::write(root.join("Cargo.toml"), b"[package]\nname = \"noise\"\n").unwrap();
}

/// Read `version` back out of the fixture's copied real manifest.json.
fn manifest_version(repo_root: &Path) -> String {
    let bytes = fs::read(repo_root.join("manifest.json")).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["version"]
        .as_str()
        .expect("manifest.json must carry a string version")
        .to_string()
}

/// A stand-in for the compiled helper binary. `validate_tree` runs it with
/// `version` (BUNDLE-006), so it has to actually execute; what this test
/// checks is tree shape, not the helper's real machine code, so a tiny
/// script filling the same contract is enough.
fn fake_helper(path: &Path, version: &str) {
    fs::write(
        path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// The plugin id grammar `omarchy-plugin-validate` enforces: the character
/// class `^[A-Za-z0-9][A-Za-z0-9._-]*$`, plus its separate `[[ $ID !=
/// *".."* ]]` substring ban. The two are independent checks in the real
/// script -- `.` is a legal character in the class, so `a..b` passes the
/// regex and still needs the substring check to fail it.
fn matches_omarchy_id_grammar(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return false;
    }
    !id.contains("..")
}

/// Kind -> required `entryPoints` key, mirroring omarchy-plugin-validate's
/// own table. A kind not listed here is left alone, same as the real script.
const KIND_ENTRY_POINTS: &[(&str, &str)] = &[
    ("bar", "bar"),
    ("bar-widget", "barWidget"),
    ("menu", "menu"),
    ("overlay", "overlay"),
    ("panel", "panel"),
    ("service", "service"),
];

/// `find`-equivalent walk: every symlink under `root`. Unlike the shell
/// tool's `find $DIR -name .git -prune -o -type l -print`, which prunes any
/// `.git` directory in the tree, this walk only skips a `.git` at the root,
/// matching `BundleValidator`'s deliberately narrower tolerance (see
/// `is_root_git_dir` in `src/plugin/bundle.rs`).
fn find_symlinks(root: &Path) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                hits.push(path);
                continue;
            }
            if dir == root && entry.file_name() == OsStr::new(".git") {
                continue;
            }
            if meta.file_type().is_dir() {
                stack.push(path);
            }
        }
    }
    hits
}

fn stamp_fake_root(dir: &Path) -> (PathBuf, agent_bar::plugin::bundle::BundleReceipt) {
    let repo_root = dir.join("repo");
    fake_repo(&repo_root);
    let version = manifest_version(&repo_root);
    let helper = dir.join("agent-bar");
    fake_helper(&helper, &version);
    let builder = BundleBuilder::new(version, "0".repeat(40)).unwrap();
    let receipt = builder.stamp(&repo_root, &helper).unwrap();
    (repo_root, receipt)
}

#[test]
fn stamped_root_mirrors_omarchy_plugin_validate() {
    let dir = tempfile::tempdir().unwrap();
    let (root, receipt) = stamp_fake_root(dir.path());

    let manifest_bytes = fs::read(root.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();

    // schemaVersion must be exactly the JSON number 1.
    assert_eq!(manifest["schemaVersion"], serde_json::json!(1));

    // id grammar and the reserved omarchy.* namespace.
    let id = manifest["id"].as_str().expect("id must be a string");
    assert!(
        matches_omarchy_id_grammar(id),
        "id '{id}' fails the omarchy id grammar"
    );
    assert!(
        !id.starts_with("omarchy."),
        "id '{id}' uses the reserved omarchy.* namespace"
    );

    // kinds must be a non-empty array.
    let kinds = manifest["kinds"]
        .as_array()
        .expect("kinds must be an array");
    assert!(!kinds.is_empty(), "kinds must be non-empty");

    // Every entry point is a safe relative path that exists on disk.
    let entry_points = manifest["entryPoints"]
        .as_object()
        .expect("entryPoints must be an object");
    assert!(!entry_points.is_empty(), "entryPoints must be non-empty");
    for (kind, ep) in entry_points {
        let rel = ep
            .as_str()
            .unwrap_or_else(|| panic!("entryPoints.{kind} must be a string"));
        assert!(!rel.starts_with('/'), "entry point must be relative: {rel}");
        assert!(
            !rel.contains(".."),
            "entry point may not contain '..': {rel}"
        );
        assert!(
            root.join(rel).is_file(),
            "entry point file not found: {rel}"
        );
    }

    // A kind is a promise to supply something to load: for every kind the
    // real script's table maps to an entry point key, that key must be
    // present. Claiming a kind without its entry point installs and enables
    // fine, then does nothing -- exactly the "mirror passes, real tool
    // fails" gap this test exists to close.
    for kind in kinds {
        let kind_str = kind.as_str().expect("kind must be a string");
        if let Some((_, ep_key)) = KIND_ENTRY_POINTS.iter().find(|(k, _)| *k == kind_str) {
            assert!(
                entry_points.contains_key(*ep_key),
                "kind '{kind_str}' requires entryPoints.{ep_key}"
            );
        }
    }

    // barWidget.defaultSection, when present, is one of the enum values the
    // shell accepts.
    let default_section = manifest["barWidget"]["defaultSection"]
        .as_str()
        .expect("barWidget.defaultSection must be present");
    assert!(
        matches!(default_section, "left" | "center" | "right"),
        "barWidget.defaultSection must be left, center, or right, got {default_section}"
    );

    // No symlinks anywhere within the shipped scope of a freshly stamped root.
    assert!(
        find_symlinks(&root.join("components")).is_empty(),
        "freshly stamped components/ must contain zero symlinks"
    );

    // README/LICENSE/preview at root, and every one of them accounted for
    // in the receipt inventory.
    for name in ["README.md", "LICENSE", "preview.png"] {
        assert!(
            root.join(name).is_file(),
            "{name} missing from stamped root"
        );
        assert!(
            receipt.files.iter().any(|f| f.path == name),
            "{name} missing from bundle.json files"
        );
    }

    // Receipt inventories the shipped scope only -- never the source tree.
    for f in &receipt.files {
        assert!(
            agent_bar::plugin::bundle::SHIPPED_ROOT_FILES.contains(&f.path.as_str())
                || agent_bar::plugin::bundle::SHIPPED_DIRS
                    .iter()
                    .any(|d| f.path.starts_with(&format!("{d}/"))),
            "non-shipped file in receipt: {}",
            f.path
        );
    }
    assert!(receipt.files.iter().all(|f| f.path != "src/lib.rs"));
    assert!(receipt.files.iter().any(|f| f.path == "bin/agent-bar"));
    assert!(receipt.files.iter().any(|f| f.path == "preview.png"));

    // The tree also satisfies our own receipt/filesystem contract.
    BundleValidator::validate_tree(&root).unwrap();
}

#[test]
fn validate_tree_tolerates_root_git_but_not_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let (root, _receipt) = stamp_fake_root(dir.path());

    // A root `.git`, like the one an installed clone carries, does not
    // break validation.
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/config"), b"[core]\n\tbare = false\n").unwrap();
    BundleValidator::validate_tree(&root).unwrap();

    // A symlink inside the shipped scope still fails, .git tolerance or
    // not. A symlink under `src/` is not `validate_tree`'s job -- the
    // shell's own validator covers the full tree at install time.
    std::os::unix::fs::symlink("/etc/passwd", root.join("components/evil-link")).unwrap();
    assert!(BundleValidator::validate_tree(&root).is_err());
}
