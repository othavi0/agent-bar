import QtQuick
import QtTest
import "../../assets/omarchy/ServiceCore.js" as Core

TestCase {
  id: testCase
  name: "AgentBarProviderStates"
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

  function loadFixture(name) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/tests/fixtures/status-v2/" + name, false)
    xhr.send()
    return JSON.parse(String(xhr.responseText))
  }

  function firstProvider(fixtureName) {
    var env = loadFixture(fixtureName)
    return env.providers[0]
  }

  function test_ready_windows_mode() {
    var p = firstProvider("ready.json")
    compare(Core.contentMode(p), "windows")
    compare(Core.connectionLabel(p.state), "Connected")
    verify(Core.planBadge(p).length > 0)
    var lines = Core.windowDisplayLines(p, "remaining")
    verify(lines.length > 0)
    verify(lines[0].percentText.indexOf("%") >= 0)
  }

  function test_empty_windows_ready_message() {
    var p = firstProvider("valid-empty-windows.json")
    compare(Core.contentMode(p), "empty_windows")
    compare(Core.stateBody(p), Core.emptyWindowsMessage())
    verify(Core.stateBody(p).indexOf("Percentage usage is not available") >= 0)
  }

  function test_stale_retains_windows_and_label() {
    var p = firstProvider("valid-stale.json")
    compare(Core.contentMode(p), "stale_windows")
    compare(Core.connectionLabel(p.state), "Stale")
    compare(Core.stateTitle(p), "Stale")
    verify(Core.errorMessage(p).length > 0)
    var acts = Core.stateActions(p)
    var kinds = acts.map(function (a) { return a.kind })
    verify(kinds.indexOf("retry") >= 0)
    var lines = Core.windowDisplayLines(p, "used")
    compare(lines.length, 1)
    compare(lines[0].percentText, "90%")
  }

  function test_cli_missing_view_installation_and_check_again() {
    var p = firstProvider("valid-cli-missing.json")
    compare(Core.contentMode(p), "state")
    compare(Core.connectionLabel(p.state), "CLI missing")
    var acts = Core.stateActions(p)
    var kinds = acts.map(function (a) { return a.kind })
    verify(kinds.indexOf("view_installation") >= 0)
    verify(kinds.indexOf("retry") >= 0)
    var labels = acts.map(function (a) { return a.label })
    verify(labels.indexOf("Check again") >= 0 || labels.join(" ").indexOf("Check") >= 0)
  }

  function test_unauthenticated_connect_or_install() {
    var p = firstProvider("valid-unauthenticated.json")
    compare(Core.contentMode(p), "state")
    compare(Core.connectionLabel(p.state), "Not connected")
    var acts = Core.stateActions(p)
    verify(acts.length >= 1)
    var kind = acts[0].kind
    verify(kind === "login" || kind === "view_installation")
    // Label should be user-facing Connect when login
    if (kind === "login")
      verify(acts[0].label.length > 0)
  }

  function test_network_and_rate_limit_are_retryable_not_auth() {
    var net = firstProvider("valid-network-error.json")
    var rate = firstProvider("valid-rate-limited.json")
    compare(Core.connectionLabel(net.state), "Network error")
    compare(Core.connectionLabel(rate.state), "Rate limited")
    verify(Core.stateBody(net).toLowerCase().indexOf("auth") < 0)
    verify(Core.stateBody(rate).toLowerCase().indexOf("auth") < 0)
    verify(Core.stateBody(net).toLowerCase().indexOf("sign in") < 0)
    var netActs = Core.stateActions(net).map(function (a) { return a.kind })
    var rateActs = Core.stateActions(rate).map(function (a) { return a.kind })
    verify(netActs.indexOf("retry") >= 0)
    verify(rateActs.indexOf("retry") >= 0)
    verify(netActs.indexOf("login") < 0)
    verify(rateActs.indexOf("login") < 0)
  }

  function test_provider_error_plain_safe_message() {
    var p = firstProvider("valid-provider-error.json")
    compare(Core.contentMode(p), "state")
    var msg = Core.errorMessage(p)
    verify(msg.length > 0)
    verify(msg.indexOf("<") < 0)
    verify(msg.indexOf("\u001b") < 0)
    verify(!Core.containsMoneyCopy(msg))
  }

  function test_loading_skeleton_mode() {
    var p = Core.placeholderProvider("claude")
    compare(p.state, "loading")
    compare(Core.contentMode(p), "skeleton")
    compare(Core.contentMode(null), "skeleton")
  }

  function test_header_model_fields() {
    var p = firstProvider("ready.json")
    var h = Core.headerModel(p, true)
    verify(h.name.length > 0)
    verify(h.plan.length > 0)
    compare(h.connection, "Connected")
    compare(h.refreshing, true)
    compare(h.showStale, false)
  }

  function test_plain_text_strips_ansi_and_controls() {
    var cleaned = Core.plainText("ok\u001b[31mred\u0007")
    verify(cleaned.indexOf("\u001b") < 0)
    verify(cleaned.indexOf("\u0007") < 0)
    verify(cleaned.indexOf("ok") >= 0)
  }

  function test_money_detector() {
    verify(Core.containsMoneyCopy("balance $12"))
    verify(Core.containsMoneyCopy("BRL 10"))
    verify(Core.containsMoneyCopy("remaining credits"))
    verify(!Core.containsMoneyCopy("Claude remaining 58%"))
  }

  function test_resolve_selected_provider() {
    var env = loadFixture("valid-multi-provider.json")
    var p = Core.resolveSelectedProvider(env, "codex", null)
    verify(p !== null)
    compare(p.id, "codex")
    var first = Core.resolveSelectedProvider(env, "", null)
    verify(first !== null)
    compare(first.id, "claude")
  }

  function test_dispatch_action_kind_mapping() {
    compare(Core.mapActionKind("retry"), "retry")
    compare(Core.mapActionKind("login"), "login")
    compare(Core.mapActionKind("view_installation"), "view_installation")
    compare(Core.mapActionKind("shell"), null)
  }
}
