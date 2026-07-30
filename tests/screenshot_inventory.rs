//! `tests/qml/TestPalette.js` and `scripts/verify-v10-ui` each hand-copy the
//! same required-screenshot inventory. Nothing keeps the two lists in sync:
//! adding a name to one and forgetting the other either leaves a gap in the
//! QML fixture's own count check or makes the shell script reject the new
//! PNG as "unexpected evidence" (exactly what happened when `ready-white.png`
//! was added to the JS list without updating the script). This test keeps
//! them in lock-step: a screenshot added to one side and forgotten on the
//! other fails here instead of at whatever point someone next runs the
//! script.

use std::collections::BTreeSet;

/// Quoted `"name.png"` basenames from `TestPalette.js`'s
/// `requiredScreenshotNames()` array.
fn js_required_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        let name = &rest[..end];
        if name.ends_with(".png") {
            names.insert(name.to_owned());
        }
    }
    names
}

/// Bare `name.png` basenames from `verify-v10-ui`'s `REQUIRED=( ... )` array.
fn script_required_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_array = false;
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("REQUIRED=(") {
            in_array = true;
            continue;
        }
        if !in_array {
            continue;
        }
        if t == ")" {
            break;
        }
        if t.ends_with(".png") && !t.contains('*') && !t.contains('"') {
            names.insert(t.to_owned());
        }
    }
    names
}

#[test]
fn screenshot_inventory_matches_verify_script() {
    let js = std::fs::read_to_string("tests/qml/TestPalette.js").expect("read TestPalette.js");
    let script =
        std::fs::read_to_string("scripts/verify-v10-ui").expect("read scripts/verify-v10-ui");

    let js_names = js_required_names(&js);
    let script_names = script_required_names(&script);

    assert!(
        !js_names.is_empty(),
        "expected at least one required screenshot name in TestPalette.js"
    );

    let missing_from_script: Vec<&String> = js_names.difference(&script_names).collect();
    let missing_from_js: Vec<&String> = script_names.difference(&js_names).collect();

    assert!(
        missing_from_script.is_empty() && missing_from_js.is_empty(),
        "screenshot inventory drifted between TestPalette.js and \
         scripts/verify-v10-ui\n  missing from scripts/verify-v10-ui REQUIRED: {missing_from_script:?}\n  \
         missing from TestPalette.js requiredScreenshotNames(): {missing_from_js:?}"
    );
}
