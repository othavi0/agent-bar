import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core
import "../../assets/omarchy/CoreService.js" as Service

TestCase {
  id: testCase
  name: "AgentBarPopup"
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

  function test_popup_uses_keyboard_panel_and_rail() {
    var src = read("assets/omarchy/Popup.qml")
    verify(src.indexOf("KeyboardPanel") >= 0)
    verify(src.indexOf("ProviderRail") >= 0)
    verify(src.indexOf("ProviderView") >= 0)
    verify(src.indexOf("PanelKeyCatcher") >= 0)
    verify(src.indexOf("fittedContent") >= 0 || src.indexOf("maxContentWidth") >= 0)
  }

  function test_rail_is_icon_only_settings_at_bottom() {
    var src = read("assets/omarchy/ProviderRail.qml")
    verify(src.indexOf("Image") >= 0)
    verify(src.indexOf("settingsBtn") >= 0 || src.indexOf("Settings") >= 0)
    verify(src.indexOf("󰒓") >= 0)
    // No provider display-name Text labels as primary rail content
    verify(src.indexOf("modelData.name") < 0 || src.indexOf("Accessible.name") >= 0)
    // Names only via Accessible / tooltip path, not as visible Text content of the icon
    verify(src.indexOf("text: modelData.name") < 0)
    verify(src.indexOf("text: modelData") < 0)
  }

  function test_header_has_no_provider_icon() {
    var src = read("assets/omarchy/components/ProviderHeader.qml")
    verify(src.indexOf("Image") < 0)
    verify(src.indexOf("iconSource") < 0)
    verify(src.indexOf("name") >= 0)
    verify(src.indexOf("plan") >= 0)
    verify(src.indexOf("󰑐") >= 0)
    // Connection state is implied structurally (windows render only when
    // ready) and update age lives in ProviderView's stale banner (plan 03
    // deleted the meta footer) — the header no longer owns that prop/text.
    verify(src.indexOf("connection") < 0)
    // §6: the plan pill becomes an uppercase tag — no more pill radius.
    verify(src.indexOf("AllUppercase") >= 0)
    verify(src.indexOf("radius: height / 2") < 0)
  }

  function test_rail_has_no_own_frame() {
    var rail = read("assets/omarchy/ProviderRail.qml")
    // §6: the rail draws no fill or border of its own inside an already
    // bordered card; the selected plate is the only chrome.
    verify(rail.indexOf("normalFill") < 0)
    verify(rail.indexOf("normalBorderColor") < 0)
    verify(rail.indexOf("selectedFill") >= 0)
    verify(rail.indexOf("anchors.topMargin: Style.spacing.popupPadding") >= 0)
    // The frame deletion's failure shape is a dangling id — runtime-only,
    // invisible to qmllint/qmltestrunner/plugin-validate (all three verified
    // blind to it). Ban the id and any anchor to it outright.
    verify(rail.indexOf("id: frame") < 0)
    verify(rail.indexOf("anchors.fill: frame") < 0)
  }

  function test_content_and_rail_share_inset_token() {
    var popup = read("assets/omarchy/Popup.qml")
    verify(popup.indexOf("contentMargins: Style.spacing.popupPadding") >= 0)
    verify(popup.indexOf("Style.space(14)") < 0)
  }

  function test_no_meta_footer() {
    // NOTE: repoRoot (above) already pops to the repo root, matching every
    // sibling read() call in this file ("assets/omarchy/..."); the brief's
    // literal "../../assets/omarchy/ProviderView.qml" resolves outside the
    // repo, so XHR returns "" and both new tests would pass/fail vacuously
    // forever regardless of ProviderView.qml's contents. Fixed to match the
    // established convention.
    var view = read("assets/omarchy/ProviderView.qml")
    // §6/§9: the meta footer is removed in all states; its age moved to the
    // stale banner and connection state is structural.
    verify(view.indexOf("connection") < 0)
    verify(view.indexOf('"Updated "') < 0)
    verify(view.indexOf('"Cache"') < 0)
    verify(view.indexOf('"Live"') < 0)
  }

  function test_stale_banner_carries_age_and_retry() {
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("󰅐") >= 0)
    verify(view.indexOf('"Last data "') >= 0)
    verify(view.indexOf("formatAgoText") >= 0)
    verify(view.indexOf("⌛") < 0)
    // formatAgoText already returns "5m ago"/"just now" — an appended " ago"
    // literal is the regression shape this test exists to catch.
    verify(view.indexOf('+ " ago"') < 0)
    verify(view.indexOf('"Last data " + (age.length ? age : "unknown")') >= 0)
  }

  function test_full_width_separator_present() {
    var src = read("assets/omarchy/ProviderView.qml")
    // Separator is the host's PanelSeparator (fixed 1px height internally);
    // this file only needs to place it full-width.
    verify(src.indexOf("PanelSeparator") >= 0)
    verify(src.indexOf("width: parent.width") >= 0)
  }

  function test_plain_text_only_no_rich_text() {
    var files = [
      "assets/omarchy/Popup.qml",
      "assets/omarchy/ProviderRail.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/StateMessage.qml"
    ]
    for (var i = 0; i < files.length; i++) {
      var src = read(files[i])
      verify(src.indexOf("Text.RichText") < 0, files[i])
      verify(src.indexOf("RichText") < 0, files[i])
      verify(src.indexOf("innerHTML") < 0, files[i])
      // Prefer explicit PlainText when setting format
      if (src.indexOf("textFormat:") >= 0)
        verify(src.indexOf("Text.PlainText") >= 0, files[i] + " should use PlainText")
    }
  }

  function test_no_money_copy_in_popup_sources() {
    var files = [
      "assets/omarchy/Popup.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/CoreView.js"
    ]
    for (var i = 0; i < files.length; i++) {
      var src = read(files[i])
      // Allow the money detector regex itself in CoreView.js
      if (files[i].indexOf("CoreView.js") >= 0)
        continue
      verify(!Core.containsMoneyCopy(src), files[i] + " has money copy")
    }
  }

  function test_card_height_accounts_for_border_inset() {
    // KeyboardPanel subtracts verticalContentInset (padding + top/bottom
    // border) from the inner area; sizing the card with padding alone leaves
    // the border as phantom overflow that enables a few-pixel scroll.
    var src = read("assets/omarchy/Popup.qml")
    verify(src.indexOf("verticalContentInset") >= 0)
    verify(src.indexOf("padding * 2") < 0)
  }

  function test_popup_open_owner_gate() {
    var ownerA = { id: "a" }
    var ownerB = { id: "b" }
    verify(!Service.popupOpenForOwner(null, ownerA))
    verify(Service.popupOpenForOwner({ owner: ownerA, providerId: "claude", view: "usage" }, ownerA))
    verify(!Service.popupOpenForOwner({ owner: ownerA, providerId: "claude", view: "usage" }, ownerB))
  }

  function test_bar_widget_hosts_popup() {
    var src = read("assets/omarchy/BarWidget.qml")
    verify(src.indexOf("Popup") >= 0)
    verify(src.indexOf("agentService") >= 0)
  }

  function test_actions_are_allowlisted_kinds_only() {
    var p = {
      id: "claude",
      name: "Claude",
      state: "cli_missing",
      windows: [],
      action: {
        kind: "view_installation",
        label: "Install guide",
        target: "https://example.com/install"
      }
    }
    var acts = Core.stateActions(p)
    for (var i = 0; i < acts.length; i++) {
      verify(Core.mapActionKind(acts[i].kind) !== null)
    }
    compare(Core.mapActionKind("rm -rf"), null)
    compare(Core.mapActionKind("bash"), null)
  }
}
