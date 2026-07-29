import QtQuick
import QtTest
import "../../assets/omarchy/CoreScroll.js" as Core

TestCase {
  id: testCase
  name: "AgentBarKeyboard"
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

  function test_provider_jk_and_arrows_delta() {
    var ids = ["claude", "codex", "amp", "grok"]
    compare(Core.routeProviderDelta(ids, "claude", 1), "codex")
    compare(Core.routeProviderDelta(ids, "grok", 1), "claude")
    compare(Core.routeProviderDelta(ids, "claude", -1), "grok")
    compare(Core.routeProviderDelta(ids, "codex", -1), "claude")
  }

  function test_text_keys_s_r_jk() {
    compare(Core.routePanelTextKey("s", false).action, "openSettings")
    compare(Core.routePanelTextKey("r", false).action, "refresh")
    compare(Core.routePanelTextKey("j", false).delta, 1)
    compare(Core.routePanelTextKey("k", false).delta, -1)
  }

  function test_editor_suppresses_shortcuts() {
    compare(Core.panelShortcutsBlocked(true), true)
    compare(Core.routePanelTextKey("s", true).action, "noop")
    compare(Core.routePanelTextKey("r", true).action, "noop")
    compare(Core.routePanelTextKey("j", true).action, "noop")
  }

  function test_focus_cycles() {
    compare(Core.focusNextIndex(0, 1, 4), 1)
    compare(Core.focusNextIndex(3, 1, 4), 0)
    compare(Core.focusNextIndex(0, -1, 4), 3)
    compare(Core.focusNextIndex(-1, 1, 4), 0)
    compare(Core.focusNextIndex(0, 1, 0), -1)
  }

  function test_popup_wires_panel_key_catcher_and_focus_controller() {
    var src = read("assets/omarchy/Popup.qml")
    verify(src.indexOf("KeyboardPanel") >= 0)
    verify(src.indexOf("PanelKeyCatcher") >= 0)
    verify(src.indexOf("FocusController") >= 0)
    verify(src.indexOf("onTabRequested") >= 0)
    verify(src.indexOf("onActivateRequested") >= 0)
    verify(src.indexOf("onCloseRequested") >= 0)
    verify(src.indexOf("onMoveRequested") >= 0)
    verify(src.indexOf("onTextKey") >= 0)
    verify(src.indexOf("editorActive") >= 0 || src.indexOf("blocked:") >= 0)
    verify(src.indexOf("PgDown") >= 0 || src.indexOf("Page Down") >= 0)
    verify(src.indexOf("scrollHome") >= 0 || src.indexOf("Home") >= 0)
  }

  // Live Quattro KeyboardPanel default property is contentItem (QQuickItem list).
  // Top-level Shortcut / Component children fail type checks on contentItem.
  function test_popup_shortcuts_nested_under_item_not_panel_default() {
    var src = read("assets/omarchy/Popup.qml")
    verify(src.indexOf("scrollShortcuts") >= 0)
    verify(src.indexOf("id: scrollShortcuts") >= 0)
    // Shortcuts must appear after PanelKeyCatcher opens, not as bare panel children.
    var keyCatcherAt = src.indexOf("id: keyCatcher")
    var firstShortcutAt = src.indexOf("Shortcut {")
    verify(keyCatcherAt >= 0)
    verify(firstShortcutAt > keyCatcherAt)
    // Loader views must be property Component, not default content children.
    verify(src.indexOf("property Component providerContent") >= 0)
    verify(src.indexOf("property Component settingsContent") >= 0)
  }

  function test_focus_controller_source() {
    var src = read("assets/omarchy/components/FocusController.qml")
    verify(src.indexOf("function move") >= 0)
    verify(src.indexOf("function activate") >= 0)
    verify(src.indexOf("ensureVisible") >= 0)
    verify(src.indexOf("scrollPage") >= 0)
    verify(src.indexOf("Behavior") < 0)
    verify(src.indexOf("Transition") < 0)
  }
}
