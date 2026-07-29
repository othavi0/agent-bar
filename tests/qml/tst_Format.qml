import QtQuick
import QtTest
import "../../assets/omarchy/ServiceCore.js" as Core

TestCase {
  name: "AgentBarFormat"

  // 2026-07-28T15:00:00Z as fixed "now".
  readonly property double nowMs: Date.parse("2026-07-28T15:00:00Z")

  function test_reset_under_1h() {
    // 37 minutes ahead.
    var text = Core.formatResetText("2026-07-28T15:37:00Z", nowMs)
    verify(text.indexOf("37m") === 0, "countdown: " + text)
    verify(text.indexOf("\u00b7") > 0, "has absolute separator: " + text)
  }

  function test_reset_under_24h_has_hours_minutes() {
    var text = Core.formatResetText("2026-07-28T17:30:00Z", nowMs)
    verify(text.indexOf("2h 30m") === 0, text)
  }

  function test_reset_over_24h_uses_days_and_weekday() {
    var text = Core.formatResetText("2026-07-31T09:00:00Z", nowMs)
    verify(text.indexOf("2d 18h") === 0, text)
    // Absolute part carries a weekday token (locale en).
    verify(/[A-Z][a-z]{2} \d\d:\d\d$/.test(text), "weekday absolute: " + text)
  }

  function test_reset_past_is_now() {
    compare(Core.formatResetText("2026-07-28T14:00:00Z", nowMs), "now")
  }

  function test_reset_invalid_is_empty() {
    compare(Core.formatResetText("", nowMs), "")
    compare(Core.formatResetText("garbage", nowMs), "")
    compare(Core.formatResetText(null, nowMs), "")
  }

  function test_ago_variants() {
    compare(Core.formatAgoText("2026-07-28T14:59:30Z", nowMs), "just now")
    compare(Core.formatAgoText("2026-07-28T14:55:00Z", nowMs), "5m ago")
    compare(Core.formatAgoText("2026-07-28T12:00:00Z", nowMs), "3h ago")
    compare(Core.formatAgoText("2026-07-26T12:00:00Z", nowMs), "2d ago")
    compare(Core.formatAgoText("nope", nowMs), "")
  }
}
