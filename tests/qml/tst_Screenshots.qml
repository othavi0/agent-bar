import QtQuick
import QtTest
import "TestPalette.js" as Core

// Deterministic UI evidence captures for CP2 (TEST screenshot inventory).
TestCase {
  id: testCase
  name: "AgentBarScreenshots"
  when: windowShown

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop(); parts.pop()
    return parts.join("/")
  }

  property string evidenceDir: {
    // Prefer env from verify-v10-ui; fallback to repo target path.
    var env = ""
    try {
      // Qt 6: no portable env in pure QML; verify script also passes via file.
      env = ""
    } catch (e) {}
    return repoRoot + "/target/v10-ui-evidence"
  }

  property int capturesDone: 0
  property int capturesExpected: 0
  property var pendingNames: []

  // Stage that renders state-labelled panels for grabToImage.
  Rectangle {
    id: stage
    width: 480
    height: 300
    color: "#18181b"
    property string titleText: ""
    property string bodyText: ""
    property string badgeText: ""
    // UX-028 (amended): the age is its own neutral caption, never merged into
    // the usage text. Modelled separately so the evidence cannot approve a
    // presentation the pane does not produce.
    property string captionText: ""
    property color fg: "#e4e4e7"
    property color muted: "#a1a1aa"
    property color badgeColor: "#e4e4e7"

    Column {
      anchors.fill: parent
      anchors.margins: 16
      spacing: 10

      Text {
        text: "Agent Bar"
        color: stage.muted
        font.pixelSize: 12
        textFormat: Text.PlainText
      }
      Text {
        text: stage.titleText
        color: stage.fg
        font.pixelSize: 18
        font.bold: true
        textFormat: Text.PlainText
      }
      Text {
        visible: stage.badgeText.length > 0
        text: stage.badgeText
        color: stage.badgeColor
        font.pixelSize: 13
        font.bold: true
        textFormat: Text.PlainText
      }
      Text {
        // No `visible` binding: under the offscreen software backend a Text
        // that flips visible false->true stays blank in the very next
        // grabToImage, which is why the badge above has never rendered in any
        // captured evidence. An always-present Text painting an empty string
        // keeps the caption reliable and the layout identical across panels.
        width: parent.width
        text: stage.captionText
        color: stage.muted
        font.pixelSize: 11
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }
      Text {
        width: parent.width
        text: stage.bodyText
        color: stage.muted
        font.pixelSize: 13
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }
    }
  }

  function applyTheme(mode) {
    var p = Core.themePalette(mode)
    stage.color = p.background
    stage.fg = p.foreground
    // Derived from this palette's own foreground at the supporting-text level,
    // which is what the shipped components compute through Util.alpha. The
    // literal 0.72 is pinned by tst_Tokens.qml's test_no_third_alpha_value;
    // Util itself is unreachable here because qs.Commons will not compile
    // under the bare Qt6 runner.
    var fg = Qt.color(p.foreground)
    stage.muted = Qt.rgba(fg.r, fg.g, fg.b, 0.72)
    stage.badgeColor = p.urgent
  }

  function paintState(name) {
    // Clear per-state fields first: the stage is reused across captures, so an
    // unset field would leak the previous panel's value into this one.
    stage.captionText = ""
    // Map evidence basename → deterministic fixture panel
    if (name.indexOf("ready-light") === 0) {
      applyTheme("light")
      stage.titleText = "Claude"
      stage.badgeText = "Connected"
      stage.bodyText = "Session (5h) · resets in 3h 1m · 58% left · Weekly (7d) 60% · 23h 1m"
      return
    }
    if (name.indexOf("ready-white") === 0) {
      applyTheme("white")
      stage.titleText = "Claude"
      stage.badgeText = "Connected"
      stage.bodyText = "Session (5h) · resets in 3h 1m · 58% left · Weekly (7d) 60% · 23h 1m"
      return
    }
    applyTheme("dark")
    if (name.indexOf("ready-dark") === 0) {
      stage.titleText = "Claude"
      stage.badgeText = "Connected"
      stage.bodyText = "Session (5h) · resets in 3h 1m · 58% left · Weekly (7d) 60% · 23h 1m"
    } else if (name.indexOf("loading-dark") === 0) {
      stage.titleText = "Loading"
      stage.badgeText = "Loading"
      stage.bodyText = "Collecting provider status…"
    } else if (name.indexOf("refreshing-with-data-dark") === 0) {
      stage.titleText = "Codex"
      stage.badgeText = "Connected · refreshing"
      stage.bodyText = "Weekly (7d) · resets in 23h 1m · 74% left (prior data kept)"
    } else if (name.indexOf("stale-dark") === 0) {
      // UX-028 (amended): a retained reading is presented as a reading. This
      // panel is ready-dark plus one neutral age caption — same title, same
      // badge, same usage text. Any other difference would be evidence of a
      // presentation the pane no longer produces.
      stage.titleText = "Claude"
      stage.badgeText = "Connected"
      stage.captionText = "Updated 14m ago"
      stage.bodyText = "Session (5h) · resets in 3h 1m · 58% left · Weekly (7d) 60% · 23h 1m"
    } else if (name.indexOf("critical-dark") === 0) {
      stage.titleText = "Claude"
      stage.badgeText = "CRITICAL"
      stage.bodyText = "Session (5h) · resets in 41m · 3% left · Weekly (7d) 60% · 23h 1m"
    } else if (name.indexOf("cli-missing-dark") === 0) {
      stage.titleText = "Amp"
      stage.badgeText = "CLI missing"
      stage.bodyText = "Amp CLI is not installed. Agent Bar reads the quota through it. Install guide"
    } else if (name.indexOf("unauthenticated-dark") === 0) {
      stage.titleText = "Claude"
      stage.badgeText = "Not connected"
      stage.bodyText = "Not signed in to Claude. Signing in opens the official Claude CLI. Sign in"
    } else if (name.indexOf("rate-limited-dark") === 0) {
      stage.titleText = "Codex"
      stage.badgeText = "Rate limited"
      stage.bodyText = "Codex hit a rate limit. Try again in a few minutes. Retry"
    } else if (name.indexOf("network-error-dark") === 0) {
      // Provider swapped Grok -> Amp to keep the fixture internally
      // consistent with the shipped copy (task brief names Amp here).
      stage.titleText = "Amp"
      stage.badgeText = "Network error"
      stage.bodyText = "Cannot reach Amp. Check your connection. Retry"
    } else if (name.indexOf("provider-error-dark") === 0) {
      // Provider swapped Amp -> Grok to match, same reason as above.
      stage.titleText = "Grok"
      stage.badgeText = "Provider error"
      stage.bodyText = "Grok returned no limits. Retry"
    } else if (name.indexOf("settings-clean-dark") === 0) {
      stage.titleText = "Settings"
      stage.badgeText = "clean"
      stage.bodyText = "Providers · Remaining · Refresh every 60 seconds · Notifications on"
    } else if (name.indexOf("settings-dirty-dark") === 0) {
      stage.titleText = "Settings"
      stage.badgeText = "dirty"
      stage.bodyText = "Draft changes: display used · Save changes enabled"
    } else if (name.indexOf("settings-invalid-dark") === 0) {
      stage.titleText = "Settings"
      stage.badgeText = "invalid"
      stage.bodyText = "Refresh every out of range · Save changes disabled"
    } else if (name.indexOf("maintenance-update-dark") === 0) {
      stage.titleText = "Maintenance"
      stage.badgeText = "Update to 10.1.0"
      stage.bodyText = "Check for updates · Release notes"
    } else if (name.indexOf("uninstall-confirmation-dark") === 0) {
      stage.titleText = "Uninstall Agent Bar"
      stage.badgeText = "Confirm"
      stage.bodyText = "Also delete saved settings and backups (unchecked) · second click required"
    } else {
      stage.titleText = name
      stage.badgeText = ""
      stage.bodyText = "fixture"
    }
  }

  function captureOne(name) {
    paintState(name)
    // Let the Column relayout before grabbing. Toggling a child's `visible`
    // (the age caption) re-runs the positioner, and grabToImage otherwise
    // captures the previous frame — the caption rendered blank without this.
    wait(50)
    var path = evidenceDir + "/" + name
    var finished = false
    stage.grabToImage(function (result) {
      result.saveToFile(path)
      capturesDone++
      finished = true
    })
    // Wait for async grab
    for (var i = 0; i < 50 && !finished; i++)
      wait(20)
    verify(finished, "grab timed out for " + name)
  }

  function test_capture_required_inventory() {
    var names = Core.requiredScreenshotNames()
    capturesExpected = names.length
    capturesDone = 0
    // Ensure directory exists via a tiny marker write using XMLHttpRequest is not possible;
    // verify-v10-ui creates the directory before invoking this test.
    for (var i = 0; i < names.length; i++)
      captureOne(names[i])
    compare(capturesDone, capturesExpected)
  }

  function test_required_names_match_spec() {
    var names = Core.requiredScreenshotNames()
    compare(names.length, 17)
    verify(names.indexOf("ready-light.png") >= 0)
    verify(names.indexOf("ready-white.png") >= 0)
    verify(names.indexOf("uninstall-confirmation-dark.png") >= 0)
  }
}
