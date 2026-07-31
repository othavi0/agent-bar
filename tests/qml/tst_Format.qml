import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core

TestCase {
  name: "AgentBarFormat"

  // 2026-07-28T15:00:00Z as fixed "now".
  readonly property double nowMs: Date.parse("2026-07-28T15:00:00Z")

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop(); parts.pop()
    return parts.join("/")
  }

  function read(rel) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/" + rel, false)
    xhr.send()
    return String(xhr.responseText || "")
  }

  function test_reset_countdown_under_1h() {
    compare(Core.resetCountdownText("2026-07-28T15:37:00Z", nowMs), "37m")
  }

  function test_reset_countdown_under_24h_keeps_hours() {
    compare(Core.resetCountdownText("2026-07-28T17:30:00Z", nowMs), "2h 30m")
  }

  function test_reset_countdown_over_24h_uses_days() {
    compare(Core.resetCountdownText("2026-07-31T09:00:00Z", nowMs), "2d 18h")
  }

  function test_reset_countdown_past_is_now() {
    compare(Core.resetCountdownText("2026-07-28T14:00:00Z", nowMs), "now")
  }

  function test_reset_countdown_invalid_is_empty() {
    compare(Core.resetCountdownText("", nowMs), "")
    compare(Core.resetCountdownText("garbage", nowMs), "")
    compare(Core.resetCountdownText(null, nowMs), "")
  }

  // The lead window's label line reads "Session (5h) \u00b7 resets in 3h 1m", and
  // "resets in now" is not English.
  function test_reset_phrase_follows_the_countdown() {
    compare(Core.resetPhrase("3h 1m"), "resets in")
    compare(Core.resetPhrase("now"), "resets")
    compare(Core.resetPhrase(""), "")
  }

  function test_ago_variants() {
    compare(Core.formatAgoText("2026-07-28T14:59:30Z", nowMs), "just now")
    compare(Core.formatAgoText("2026-07-28T14:55:00Z", nowMs), "5m ago")
    compare(Core.formatAgoText("2026-07-28T12:00:00Z", nowMs), "3h ago")
    compare(Core.formatAgoText("2026-07-26T12:00:00Z", nowMs), "2d ago")
    compare(Core.formatAgoText("nope", nowMs), "")
  }

  // The other half of the Rust seam: same table, same expectations, read from
  // the same file. See tests/countdown_parity.rs.
  function test_countdown_matches_the_shared_table() {
    var rows = JSON.parse(read("tests/fixtures/countdown-table.json"))
    verify(rows.length >= 12, "the shared table must not shrink")
    for (var i = 0; i < rows.length; i++) {
      compare(Core.countdownText(rows[i].minutes * 60000), rows[i].text,
              "minutes = " + rows[i].minutes)
    }
  }
}
