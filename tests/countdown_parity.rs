//! `CoreView.countdownText` humanises a reset countdown in QML; the
//! notification path humanises the same durations in Rust. The repo bans every
//! JS runtime, so a Rust test cannot call the QML function — instead both
//! sides are pinned to one shared table of inputs and expected outputs.
//! `tests/qml/tst_Format.qml` asserts the QML implementation against the same
//! file, so either side drifting fails its own suite.

use agent_bar::support::countdown::countdown_text;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    minutes: i64,
    text: String,
}

fn table() -> Vec<Row> {
    let raw = std::fs::read_to_string("tests/fixtures/countdown-table.json")
        .expect("read countdown-table.json");
    serde_json::from_str(&raw).expect("parse countdown-table.json")
}

#[test]
fn countdown_matches_the_shared_table() {
    let rows = table();
    assert!(
        rows.len() >= 12,
        "the table must keep covering both sides of every branch boundary"
    );
    for row in rows {
        assert_eq!(
            countdown_text(time::Duration::minutes(row.minutes)),
            row.text,
            "minutes = {}",
            row.minutes
        );
    }
}

// The seam is only real while the QML side still exists under that name.
#[test]
fn qml_countdown_function_still_exists() {
    let js = std::fs::read_to_string("CoreView.js").expect("read CoreView.js");
    assert!(
        js.contains("function countdownText(diffMs)"),
        "CoreView.countdownText is the other half of this seam"
    );
    assert!(
        js.contains("function resetCountdownText(iso, nowMs)"),
        "CoreView.resetCountdownText is the other half of reset_countdown"
    );
}
