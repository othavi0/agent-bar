//! Distribution-tree completeness (git-plugin-distribution Task 6).
//!
//! The assembled `agent-bar.usage/` tree is pushed verbatim as the
//! distribution repo's complete state, so it has to satisfy two things at
//! once: the Omarchy plugin contract the shell enforces at install time, and
//! the marketplace expectation of a README/LICENSE/preview at the repo root.
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

/// Build a throwaway "source repo" containing everything `assemble` reads.
///
/// `assets/omarchy` is the real tree, so the manifest and entry points this
/// test checks are the ones that actually ship. The terminal helper, dist
/// README, LICENSE, and preview image are small fakes: their exact bytes are
/// not under test here, only that `assemble` picks them up and the resulting
/// tree is contract-complete.
fn fake_repo(root: &Path) {
    copy_dir_all(
        &workspace_root().join("assets/omarchy"),
        &root.join("assets/omarchy"),
    );

    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/agent-bar-open-terminal"),
        b"#!/bin/bash\nexit 0\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("assets/dist")).unwrap();
    fs::write(root.join("assets/dist/README.md"), b"# Fake dist readme\n").unwrap();

    fs::write(root.join("LICENSE"), b"Fake license text.\n").unwrap();

    fs::create_dir_all(root.join("docs/media")).unwrap();
    fs::write(
        root.join("docs/media/demo.png"),
        b"not a real png, just stand-in bytes",
    )
    .unwrap();
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

fn assemble_fake_tree(dir: &Path) -> (PathBuf, agent_bar::plugin::bundle::BundleReceipt) {
    let repo_root = dir.join("repo");
    fake_repo(&repo_root);

    let version = "10.3.0";
    let helper = dir.join("agent-bar");
    fake_helper(&helper, version);

    let out = dir.join("agent-bar.usage");
    let builder = BundleBuilder::new(version, "0".repeat(40)).unwrap();
    let receipt = builder.assemble(&out, &repo_root, &helper).unwrap();
    (out, receipt)
}

#[test]
fn assembled_tree_mirrors_omarchy_plugin_validate() {
    let dir = tempfile::tempdir().unwrap();
    let (out, receipt) = assemble_fake_tree(dir.path());

    let manifest_bytes = fs::read(out.join("manifest.json")).unwrap();
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
        assert!(out.join(rel).is_file(), "entry point file not found: {rel}");
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

    // No symlinks anywhere in a freshly assembled tree.
    assert!(
        find_symlinks(&out).is_empty(),
        "freshly assembled tree must contain zero symlinks"
    );

    // Distribution-repo completeness: README/LICENSE/preview at root, and
    // every one of them accounted for in the receipt inventory.
    for name in ["README.md", "LICENSE", "preview.png"] {
        assert!(
            out.join(name).is_file(),
            "{name} missing from assembled tree"
        );
        assert!(
            receipt.files.iter().any(|f| f.path == name),
            "{name} missing from bundle.json files"
        );
    }

    // The tree also satisfies our own receipt/filesystem contract.
    BundleValidator::validate_tree(&out).unwrap();
}

#[test]
fn validate_tree_tolerates_root_git_but_not_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let (out, _receipt) = assemble_fake_tree(dir.path());

    // A root `.git`, like the one an installed clone carries, does not
    // break validation.
    fs::create_dir_all(out.join(".git")).unwrap();
    fs::write(out.join(".git/config"), b"[core]\n\tbare = false\n").unwrap();
    BundleValidator::validate_tree(&out).unwrap();

    // A symlink elsewhere in the tree still fails, .git tolerance or not.
    std::os::unix::fs::symlink("/etc/passwd", out.join("evil-link")).unwrap();
    assert!(BundleValidator::validate_tree(&out).is_err());
}
