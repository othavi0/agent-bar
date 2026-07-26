//! Structural JSON Schema contracts for status v2 and settings v1.
//!
//! Cross-field semantic checks (percentage sum, unique IDs, helper version
//! equality, request ordering) belong to Task 3 and are intentionally out of
//! scope here.

use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::Validator;
use serde_json::Value;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

fn compile_schema(relative: &str) -> Validator {
    let path = workspace_root().join(relative);
    let schema = load_json(&path);
    jsonschema::validator_for(&schema).unwrap_or_else(|err| {
        panic!("failed to compile schema {}: {err}", path.display());
    })
}

fn fixture_paths(dir: &str) -> Vec<PathBuf> {
    let root = workspace_root().join(dir);
    let mut paths: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    paths.sort();
    paths
}

fn assert_valid(validator: &Validator, path: &Path) {
    let instance = load_json(path);
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|err| format!("{err}"))
        .collect();
    assert!(
        errors.is_empty(),
        "expected {} to be valid, but got errors:\n  - {}",
        path.display(),
        errors.join("\n  - ")
    );
}

fn assert_invalid(validator: &Validator, path: &Path) {
    let instance = load_json(path);
    assert!(
        !validator.is_valid(&instance),
        "expected {} to be invalid against the schema",
        path.display()
    );
}

#[test]
fn schema_contract_status_v2_valid_fixtures_pass() {
    let validator = compile_schema("schemas/status-v2.schema.json");
    let ready = workspace_root().join("tests/fixtures/status-v2/ready.json");
    assert_valid(&validator, &ready);

    for path in fixture_paths("tests/fixtures/status-v2") {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("valid-") || name == "ready.json" {
            assert_valid(&validator, &path);
        }
    }
}

#[test]
fn schema_contract_status_v2_invalid_fixtures_fail() {
    let validator = compile_schema("schemas/status-v2.schema.json");
    let over = workspace_root().join("tests/fixtures/status-v2/percent-over-100.json");
    let money = workspace_root().join("tests/fixtures/status-v2/money-field.json");
    assert_invalid(&validator, &over);
    assert_invalid(&validator, &money);

    for path in fixture_paths("tests/fixtures/status-v2") {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("invalid-")
            || name == "percent-over-100.json"
            || name == "money-field.json"
        {
            assert_invalid(&validator, &path);
        }
    }
}

#[test]
fn schema_contract_settings_v1_valid_fixtures_pass() {
    let validator = compile_schema("schemas/settings-v1.schema.json");
    for path in fixture_paths("tests/fixtures/settings-v1") {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("valid-") {
            assert_valid(&validator, &path);
        }
    }
}

#[test]
fn schema_contract_settings_v1_invalid_fixtures_fail() {
    let validator = compile_schema("schemas/settings-v1.schema.json");
    for path in fixture_paths("tests/fixtures/settings-v1") {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("invalid-") {
            assert_invalid(&validator, &path);
        }
    }
}

#[test]
fn schema_contract_support_seams_are_object_safe() {
    use agent_bar::support::{Clock, FileSystem, RealFileSystem, SystemClock};
    use std::path::Path;
    use std::sync::Arc;

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let _now = clock.now_utc();

    let fs: Arc<dyn FileSystem> = Arc::new(RealFileSystem);
    let cargo = workspace_root().join("Cargo.toml");
    let bytes = fs.read(Path::new(&cargo)).expect("read Cargo.toml");
    assert!(!bytes.is_empty());
    let meta = fs.metadata(Path::new(&cargo)).expect("metadata Cargo.toml");
    assert!(meta.len > 0);
}
