import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core
import "TestPalette.js" as Palette

TestCase {
  id: testCase
  name: "AgentBarAccessibility"
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

  function read(rel) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/" + rel, false)
    xhr.send()
    return String(xhr.responseText || "")
  }

  function v10QmlFiles() {
    return [
      "assets/omarchy/BarWidget.qml",
      "assets/omarchy/Popup.qml",
      "assets/omarchy/ProviderRail.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/SettingsView.qml",
      "assets/omarchy/MaintenanceView.qml",
      "assets/omarchy/Service.qml",
      "assets/omarchy/components/ProviderChip.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/components/HeaderTag.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/SettingsProviderRow.qml",
      "assets/omarchy/components/ConfirmDialog.qml",
      "assets/omarchy/components/FocusController.qml"
    ]
  }

  function test_no_plugin_authored_animations() {
    var files = v10QmlFiles()
    for (var i = 0; i < files.length; i++) {
      var src = read(files[i])
      // Strip // comments before checking tokens. Avoid matching boundsBehavior.
      var code = src.replace(/\/\/[^\n]*/g, "")
      verify(!/\bBehavior\b/.test(code), files[i] + " has Behavior")
      verify(!/\bTransition\b/.test(code), files[i] + " has Transition")
      var stripped = code.replace(/Accessible\.[A-Za-z]+/g, "")
      verify(!/\b[A-Za-z]*Animation\b/.test(stripped), files[i] + " has Animation")
      verify(!/\b[A-Za-z]*Animator\b/.test(stripped), files[i] + " has Animator")
    }
  }

  function test_no_custom_icon_button_in_v10() {
    var files = v10QmlFiles()
    for (var i = 0; i < files.length; i++) {
      var src = read(files[i])
      verify(src.indexOf("component IconButton") < 0, files[i])
      verify(src.indexOf("IconButton {") < 0, files[i])
    }
  }

  function test_interactive_controls_have_accessible_names() {
    var src = read("assets/omarchy/ProviderRail.qml")
    verify(src.indexOf("Accessible.name") >= 0)
    verify(src.indexOf("Accessible.role") >= 0)
    src = read("assets/omarchy/components/StateMessage.qml")
    verify(src.indexOf("Accessible.name") >= 0)
    src = read("assets/omarchy/SettingsView.qml")
    verify(src.indexOf("Accessible.name") >= 0)
    src = read("assets/omarchy/MaintenanceView.qml")
    verify(src.indexOf("Accessible.name") >= 0)
  }

  function test_state_cues_not_color_only() {
    // Chip and state message use text cues beyond color
    verify(Core.chipStateCue({ state: "stale" }).length > 0)
    verify(Core.chipStateCue({ state: "cli_missing" }).length > 0)
    verify(Core.stateTitle({ state: "network_error", windows: [] }).length > 0)
    compare(Core.stateQualifier("stale"), "stale")
  }

  function test_glyphs_and_text_labels() {
    var header = read("assets/omarchy/components/ProviderHeader.qml")
    verify(header.indexOf("󰑐") >= 0)
    var rail = read("assets/omarchy/ProviderRail.qml")
    verify(rail.indexOf("󰒓") >= 0)
    var settings = read("assets/omarchy/SettingsView.qml")
    verify(settings.indexOf("Save changes") >= 0)
    verify(settings.indexOf("Restore defaults") >= 0)
    var maint = read("assets/omarchy/MaintenanceView.qml")
    verify(maint.indexOf("Check for updates") >= 0)
    verify(maint.indexOf("Uninstall Agent Bar") >= 0)
  }

  function test_theme_palette_light_dark() {
    var light = Palette.themePalette("light")
    var dark = Palette.themePalette("dark")
    compare(light.mode, "light")
    compare(dark.mode, "dark")
    verify(light.background !== dark.background)
    verify(light.foreground !== dark.foreground)
  }

  function test_focus_controller_ordered_activation_api() {
    var src = read("assets/omarchy/components/FocusController.qml")
    verify(src.indexOf("function setTargets") >= 0)
    verify(src.indexOf("liveTargets") >= 0)
    verify(src.indexOf("focusActivate") >= 0)
  }

  function test_settings_editor_owns_focus_flag() {
    var src = read("assets/omarchy/SettingsView.qml")
    verify(src.indexOf("editorOwnsFocus") >= 0)
  }
}
