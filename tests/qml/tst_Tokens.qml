import QtQuick
import QtTest

TestCase {
  id: testCase
  name: "AgentBarTokens"
  when: windowShown

  property string repoRoot: {
    var path = String(Qt.resolvedUrl(".")).replace("file://", "")
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

  function tokenScannedFiles() {
    return [
      "assets/omarchy/BarWidget.qml",
      "assets/omarchy/Popup.qml",
      "assets/omarchy/ProviderRail.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/SettingsView.qml",
      "assets/omarchy/MaintenanceView.qml",
      "assets/omarchy/components/ProviderChip.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/SettingsProviderRow.qml",
      "assets/omarchy/components/ConfirmDialog.qml"
    ]
  }

  // Qt.darker divides HSV value. On a dark theme that recedes; on a light
  // theme it advances, so secondary text outranks primary. Util.alpha works
  // in both directions with one value.
  function test_no_qt_darker() {
    var files = tokenScannedFiles()
    for (var i = 0; i < files.length; i++) {
      var code = read(files[i]).replace(/\/\/[^\n]*/g, "")
      verify(code.indexOf("Qt.darker") < 0,
             files[i] + " still calls Qt.darker; use Util.alpha")
    }
  }

  // Every numeric literal that appears in the opacity slot of a
  // Util.alpha(color, opacity) call, read from source. Handles both plain
  // literals and conditional expressions (the showStale ternary).
  function alphaArgValues(code) {
    var values = []
    var callRe = /Util\.alpha\(([^()]*)\)/g
    var m
    while ((m = callRe.exec(code)) !== null) {
      var args = m[1]
      var commaIdx = args.indexOf(",")
      if (commaIdx < 0)
        continue
      var opacityArg = args.slice(commaIdx + 1)
      var numRe = /\d+(?:\.\d+)?/g
      var nm
      while ((nm = numRe.exec(opacityArg)) !== null)
        values.push(nm[0])
    }
    return values
  }

  function convertedFiles() {
    return [
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/SettingsView.qml",
      "assets/omarchy/MaintenanceView.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/ConfirmDialog.qml"
    ]
  }

  // The "exactly two levels, no third value" contract that Tasks 5-8 build
  // on. Values are extracted from the call sites themselves, not hardcoded,
  // so a future task introducing a third alpha value fails here. Does not
  // assert a call-site count: later plans legitimately add/remove sites.
  function test_no_third_alpha_value() {
    var files = tokenScannedFiles()
    var seen = {}
    for (var i = 0; i < files.length; i++) {
      var code = read(files[i]).replace(/\/\/[^\n]*/g, "")
      var values = alphaArgValues(code)
      for (var j = 0; j < values.length; j++)
        seen[values[j]] = true
    }
    var distinct = Object.keys(seen).sort()
    compare(distinct.join(","), "0.55,0.72",
            "Util.alpha opacity must be exactly 0.55 or 0.72, found: " + distinct.join(","))
  }

  // Closes the substitution hole: a hardcoded Qt.rgba(...) literal would
  // satisfy test_no_qt_darker without ever using Util.alpha.
  function test_util_alpha_used_in_converted_files() {
    var files = convertedFiles()
    for (var i = 0; i < files.length; i++) {
      var code = read(files[i]).replace(/\/\/[^\n]*/g, "")
      verify(code.indexOf("Util.alpha(") >= 0,
             files[i] + " has no Util.alpha( call; conversion must use Util.alpha")
    }
  }

  // Composite a translucent foreground over a background, the way the
  // compositor does, then compare WCAG contrast.
  function composite(fg, bg, a) {
    return Qt.rgba(fg.r * a + bg.r * (1 - a),
                   fg.g * a + bg.g * (1 - a),
                   fg.b * a + bg.b * (1 - a), 1)
  }

  function luminance(c) {
    function ch(v) { return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4) }
    return 0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
  }

  function contrast(a, b) {
    var la = luminance(a), lb = luminance(b)
    var hi = Math.max(la, lb), lo = Math.min(la, lb)
    return (hi + 0.05) / (lo + 0.05)
  }

  function test_secondary_recedes_in_both_themes_data() {
    return [
      { tag: "dark",  fg: Qt.color("#fff6ff"), bg: Qt.color("#05080a") },
      { tag: "light", fg: Qt.color("#18181b"), bg: Qt.color("#f4f4f5") },
      { tag: "white", fg: Qt.color("#000000"), bg: Qt.color("#ffffff") }
    ]
  }

  function test_secondary_recedes_in_both_themes(data) {
    var primary = contrast(data.fg, data.bg)
    var supporting = contrast(composite(data.fg, data.bg, 0.72), data.bg)
    var meta = contrast(composite(data.fg, data.bg, 0.55), data.bg)
    verify(supporting < primary,
           data.tag + ": supporting " + supporting + " must be under primary " + primary)
    verify(meta < supporting,
           data.tag + ": meta " + meta + " must be under supporting " + supporting)
  }
}
