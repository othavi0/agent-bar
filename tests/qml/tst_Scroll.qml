import QtQuick
import QtTest
import "../../CoreScroll.js" as Core

TestCase {
  id: testCase
  name: "AgentBarScroll"
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

  function test_clamp_content_y() {
    compare(Core.maxContentY(500, 200), 300)
    compare(Core.maxContentY(100, 200), 0)
    compare(Core.clampContentY(-10, 500, 200), 0)
    compare(Core.clampContentY(999, 500, 200), 300)
    compare(Core.clampContentY(50, 500, 200), 50)
  }

  function test_page_delta_viewport_minus_line() {
    compare(Core.pageScrollDelta(200, 20), 180)
    compare(Core.pageScrollDelta(20, 20), 20)
  }

  function test_page_scroll_clamps() {
    compare(Core.applyPageScroll(0, 1, 200, 500, 20), 180)
    compare(Core.applyPageScroll(250, 1, 200, 500, 20), 300)
    compare(Core.applyPageScroll(50, -1, 200, 500, 20), 0)
  }

  function test_home_end() {
    compare(Core.scrollHomeY(), 0)
    compare(Core.scrollEndY(500, 200), 300)
  }

  function test_short_content_clamp() {
    // Was scrolled deep; content shrinks
    compare(Core.clampContentY(400, 150, 200), 0)
  }

  function test_flickable_interactive_only_when_overflow() {
    verify(!Core.flickableInteractive(100, 200))
    verify(!Core.flickableInteractive(200, 200))
    verify(Core.flickableInteractive(201, 200))
    verify(Core.flickableInteractive(500, 200))
  }

  function test_fitted_popup_content_height() {
    // No large empty floor: body 120 with min 160 → 160; body 400 → 400; cap 300 → 300
    compare(Core.fittedPopupContentHeight(120, 160, 560), 160)
    compare(Core.fittedPopupContentHeight(400, 160, 560), 400)
    compare(Core.fittedPopupContentHeight(900, 160, 560), 560)
    // Old 280 floor is not required
    compare(Core.fittedPopupContentHeight(100, 160, 560), 160)
  }

  function test_content_y_for_item_reveals() {
    // Item above viewport
    compare(Core.contentYForItem(100, 200, 1000, 20, 30), 20)
    // Item below viewport
    compare(Core.contentYForItem(0, 200, 1000, 250, 40), 90)
    // Item already visible
    compare(Core.contentYForItem(0, 200, 1000, 50, 20), 0)
  }

  function test_popup_flickable_contract() {
    var src = read("Popup.qml")
    verify(src.indexOf("Flickable") >= 0)
    verify(src.indexOf("contentWidth: width") >= 0)
    verify(src.indexOf("flickableDirection: Flickable.VerticalFlick") >= 0)
    verify(src.indexOf("boundsBehavior: Flickable.StopAtBounds") >= 0)
    verify(src.indexOf("ScrollBar.vertical") >= 0)
    verify(src.indexOf("ScrollBar.AsNeeded") >= 0)
    // Overflow-gated interaction (short content must not scroll)
    verify(src.indexOf("flickableInteractive") >= 0 || src.indexOf("interactive:") >= 0)
    // No artificial 280px empty floor
    verify(src.indexOf("Style.space(280)") < 0)
    // No custom wheel inversion / network on scroll
    verify(src.indexOf("onWheel") < 0)
    verify(src.indexOf("angleDelta") < 0)
  }

  function test_bar_widget_foreign_dismiss_contract() {
    var src = read("BarWidget.qml")
    verify(src.indexOf("dismissPopup") >= 0)
    verify(src.indexOf("foreignPopupOpen") >= 0 || src.indexOf("foreignDismiss") >= 0)
  }

  function test_provider_rail_stack_no_bottom_pin() {
    var src = read("ProviderRail.qml")
    // Option A: Settings in ColumnLayout stack, not anchors.bottom over icons.
    verify(src.indexOf("ColumnLayout") >= 0)
    verify(src.indexOf("minStackHeight") >= 0)
    verify(src.indexOf("anchors.bottom: parent.bottom") < 0)
    verify(src.indexOf("border.width") >= 0)
    // Settings uses same slot size (not PanelActionButton size blow-out).
    verify(src.indexOf("settingsItem") >= 0)
    verify(src.indexOf("PanelActionButton") < 0)
  }

  function test_popup_content_width_accounts_for_gutter() {
    var src = read("Popup.qml")
    verify(src.indexOf("railGutter") >= 0)
    verify(src.indexOf("parent.width - rail.width - railGutter.width") >= 0)
    // Old off-by-one that clipped content text on the left.
    verify(src.indexOf("parent.width - rail.width - 1") < 0)
  }
}
