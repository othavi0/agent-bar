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
    // Task 4 extracted the tag shape into HeaderTag.qml (its own uppercase
    // assertion lives in test_header_renders_plan_and_severity_tags below),
    // so this file only needs to prove it delegates to that component.
    verify(src.indexOf("HeaderTag") >= 0)
    verify(src.indexOf("radius: height / 2") < 0)
  }

  // §6: name · plan tag · [severity tag] · spacer · refresh. One tag shape,
  // one urgent variant — not two hand-copied Rectangles.
  function test_header_renders_plan_and_severity_tags() {
    var hdr = read("assets/omarchy/components/ProviderHeader.qml")
    verify(hdr.indexOf("HeaderTag {") >= 0)
    verify(hdr.indexOf("id: planTag") >= 0)
    verify(hdr.indexOf("id: severityTag") >= 0)
    verify(hdr.indexOf("property string severityText") >= 0)
    verify(hdr.indexOf("property bool severityUrgent") >= 0)
    // The pill's own Rectangle is gone; the tag lives in one file now.
    verify(hdr.indexOf("border.color: Style.normalBorderColor") < 0)

    var tag = read("assets/omarchy/components/HeaderTag.qml")
    verify(tag.indexOf("Color.urgent") >= 0)
    verify(tag.indexOf("radius: Style.cornerRadius") >= 0)
    verify(tag.indexOf("Font.AllUppercase") >= 0)
    verify(tag.indexOf("Qt.rgba(") < 0)
  }

  // The refresh glyph must stay inside the pane when both tags render; the
  // spacer subtracts the real tag widths instead of a lump constant.
  function test_header_spacer_accounts_for_both_tags() {
    var hdr = read("assets/omarchy/components/ProviderHeader.qml")
    verify(hdr.indexOf("planTag.visible ? planTag.width") >= 0)
    verify(hdr.indexOf("severityTag.visible ? severityTag.width") >= 0)
    verify(hdr.indexOf("Style.space(60)") < 0,
           "the old lump constant hid the second tag's width")
    verify(hdr.indexOf("row.spacing * 2") >= 0,
           "the spacer must charge both unconditional gaps: before it and after it")
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

  // §6: the lead window is 2.5x the body size with the reset promoted into
  // its label line; the old bottom "resets" row is gone.
  function test_lead_window_geometry_and_label_line() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    verify(win.indexOf("Math.round(Style.font.body * 2.5)") >= 0)
    verify(win.indexOf("Style.font.body * 1.8") < 0)
    verify(win.indexOf("root.resetPhrase") >= 0)
    verify(win.indexOf("root.resetCountdown") >= 0)
    verify(win.indexOf("property string resetText") < 0,
           "resetText is replaced by the countdown pair")
    verify(win.indexOf('text: "resets"') < 0,
           "the reset row moved into the label line")
  }

  // UX-020A extended: every window row carries a track, not just the lead.
  function test_compact_rows_carry_their_own_track() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    var compact = win.slice(win.indexOf("id: compactRow"))
    verify(compact.length > 0, "compactRow must still exist")
    verify(compact.indexOf("color: root.trackColor") >= 0,
           "the compact row needs a track of its own (UX-020A)")
    verify(compact.indexOf("root.resetCountdown") >= 0,
           "the compact row needs its reset column")
    verify(win.indexOf('text: "23h 59m"') >= 0,
           "the reset column is measured with TextMetrics, never hardcoded px")
  }

  // §7: critical paints the numeral and the fill in Color.urgent; nothing
  // else in this file may introduce a colour.
  function test_critical_window_uses_the_urgent_token() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    verify(win.indexOf("Color.urgent") >= 0)
    verify(win.indexOf('root.severity === "critical"') >= 0)
    verify(win.indexOf("Qt.rgba(") < 0)

    // The numeral and its track must agree about severity. They disagreed
    // once: valueColor ignored `dimmed` while fillColor gave it precedence,
    // so a stale critical window painted an urgent number beside a neutral
    // track. Both must reach Color.urgent from `isCritical` alone.
    var value = win.slice(win.indexOf("readonly property color valueColor"))
    value = value.slice(0, value.indexOf("\n"))
    verify(value.indexOf("root.dimmed") < 0,
           "valueColor must not consult dimmed: " + value)
    var fill = win.slice(win.indexOf("readonly property color fillColor"))
    fill = fill.slice(0, fill.indexOf("readonly property real fillOpacity"))
    verify(fill.indexOf("root.isCritical") < fill.indexOf("root.dimmed"),
           "fillColor must test isCritical before dimmed: " + fill)
  }

  function test_provider_view_leads_with_one_window() {
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("Core.windowLayout(") >= 0)
    verify(view.indexOf("emphasis: true") >= 0)
    verify(view.indexOf("emphasis: false") >= 0)
    verify(view.indexOf("severityText: ") >= 0)
    // The quiet rule between lead and rows is gone (approved mockup).
    verify(view.indexOf("strength: 0.08") < 0)
  }

  // The three QML gates never compile a file that imports qs.*, so a dangling
  // reference to a deleted function is invisible to them. These guards ban the
  // exact dead identifiers by name.
  function test_primary_window_allowlist_is_gone() {
    var core = read("assets/omarchy/CoreView.js")
    verify(core.indexOf("PRIMARY_WINDOW_IDS") < 0,
           "the id allowlist must be deleted, not left dormant")
    verify(core.indexOf("windowGroups") < 0,
           "windowGroups is replaced by windowLayout")
    verify(core.indexOf("function electLeadIndex") >= 0)
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("groups.primary") < 0)
    verify(view.indexOf("groups.secondary") < 0)
  }

  // §6/§3.7: the popup shows the countdown only. The absolute-clock humaniser
  // is deleted with its weekday table and its only Qt.formatDateTime call.
  function test_absolute_clock_humaniser_is_gone() {
    var core = read("assets/omarchy/CoreView.js")
    verify(core.indexOf("formatResetText") < 0)
    verify(core.indexOf("WEEKDAYS") < 0)
    verify(core.indexOf("Qt.formatDateTime") < 0)
    verify(core.indexOf("function resetCountdownText") >= 0)
  }
}
