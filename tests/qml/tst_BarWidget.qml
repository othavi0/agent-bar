import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core

TestCase {
  id: testCase
  name: "AgentBarBarWidget"
  when: windowShown

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop()
    parts.pop()
    return parts.join("/")
  }

  property string widgetUrl: "file://" + repoRoot + "/assets/omarchy/BarWidget.qml"
  property string chipUrl: "file://" + repoRoot + "/assets/omarchy/components/ProviderChip.qml"

  // Minimal shell stand-in with Quattro serviceFor API.
  Item {
    id: fakeShell
    property var _services: ({})

    function serviceFor(pluginId) {
      return _services[String(pluginId)] || null
    }

    function registerService(pluginId, svc) {
      var next = ({})
      for (var k in _services)
        next[k] = _services[k]
      next[String(pluginId)] = svc
      _services = next
    }
  }

  Item {
    id: fakeBar
    property var shell: fakeShell
    property color foreground: "#ffffff"
    property string fontFamily: "monospace"
    property bool vertical: false
    property int barSize: 28
    property var clickTargets: []
    property string lastTooltip: ""
    property var lastTooltipTarget: null

    function registerClickTarget(target) {
      var next = clickTargets.slice()
      if (next.indexOf(target) < 0)
        next.push(target)
      clickTargets = next
    }

    function unregisterClickTarget(target) {
      var next = []
      for (var i = 0; i < clickTargets.length; i++) {
        if (clickTargets[i] !== target)
          next.push(clickTargets[i])
      }
      clickTargets = next
    }

    function showTooltip(target, text) {
      lastTooltipTarget = target
      lastTooltip = String(text || "")
    }

    function hideTooltip(target) {
      if (lastTooltipTarget === target) {
        lastTooltipTarget = null
        lastTooltip = ""
      }
    }
  }

  // Chip logic matching BarWidget.qml agentService resolution (without qs.Ui).
  component AgentChip: Item {
    property var bar: null
    property string moduleName: "agent-bar.usage"
    readonly property var agentService: bar && bar.shell
        ? bar.shell.serviceFor(moduleName)
        : null
  }

  function makeProvider(id, state, used, remaining, resetsAt) {
    var windows = []
    if (used !== undefined && remaining !== undefined && used !== null) {
      windows.push({
        id: "session",
        label: "Session",
        usedPercent: used,
        remainingPercent: remaining,
        resetsAt: resetsAt === undefined ? null : resetsAt
      })
    }
    return {
      id: id,
      name: Core.providerDisplayName(id),
      state: state || "ready",
      source: state === "ready" ? "live" : (state === "stale" ? "cache" : null),
      plan: null,
      account: null,
      windows: windows,
      lastSuccessAt: state === "ready" || state === "stale" ? "2026-07-26T18:42:00Z" : null,
      error: null,
      action: null
    }
  }

  function makeSnapshot(providers) {
    return {
      schemaVersion: 2,
      helperVersion: "10.0.0",
      generatedAt: "2026-07-26T18:42:00Z",
      request: { provider: null, cache: "use" },
      providers: providers
    }
  }

  // ---- Task 8 carry-over ----

  function test_two_widgets_resolve_same_service() {
    var svc = Qt.createQmlObject('import QtQuick; Item { property string helperVersion: "10.0.0"; property bool versionReady: true; property bool versionFailed: false }', testCase)
    fakeShell.registerService("agent-bar.usage", svc)

    var w1 = agentChipComp.createObject(testCase, {
      bar: fakeBar,
      moduleName: "agent-bar.usage"
    })
    var w2 = agentChipComp.createObject(testCase, {
      bar: fakeBar,
      moduleName: "agent-bar.usage"
    })
    verify(w1.agentService !== null)
    verify(w2.agentService !== null)
    compare(w1.agentService, svc)
    compare(w2.agentService, svc)
    compare(w1.agentService, w2.agentService)
    compare(w1.agentService.helperVersion, "10.0.0")

    w1.destroy()
    w2.destroy()
    svc.destroy()
  }

  function test_widget_without_shell_has_null_service() {
    var w = agentChipComp.createObject(testCase, {
      bar: null,
      moduleName: "agent-bar.usage"
    })
    compare(w.agentService, null)
    w.destroy()
  }

  function test_bar_widget_source_uses_serviceFor() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", widgetUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("serviceFor(moduleName)") >= 0)
    verify(src.indexOf("moduleName: \"agent-bar.usage\"") >= 0)
    verify(src.indexOf("Qt.resolvedUrl") >= 0) // icons only
  }

  // ---- Task 10: chip model ----

  function test_visible_providers_settings_order_and_filter() {
    var settings = {
      schemaVersion: 1,
      providers: [
        { id: "grok", enabled: true },
        { id: "claude", enabled: true },
        { id: "codex", enabled: false },
        { id: "amp", enabled: true }
      ],
      display: { metric: "remaining" },
      refreshIntervalSeconds: 60,
      notifications: { enabled: true }
    }
    var snap = makeSnapshot([
      makeProvider("claude", "ready", 10, 90),
      makeProvider("codex", "ready", 20, 80),
      makeProvider("amp", "ready", 30, 70),
      makeProvider("grok", "ready", 40, 60)
    ])
    var chips = Core.visibleProviders(snap, settings)
    compare(chips.length, 3)
    compare(chips[0].id, "grok")
    compare(chips[1].id, "claude")
    compare(chips[2].id, "amp")
  }

  function test_visible_providers_without_settings_uses_snapshot() {
    var snap = makeSnapshot([
      makeProvider("amp", "ready", 5, 95),
      makeProvider("claude", "ready", 10, 90)
    ])
    var chips = Core.visibleProviders(snap, null)
    compare(chips.length, 2)
    compare(chips[0].id, "amp")
    compare(chips[1].id, "claude")
  }

  function test_empty_windows_render_em_dash() {
    var p = makeProvider("amp", "ready")
    compare(p.windows.length, 0)
    compare(Core.chipPercentText(p, "remaining"), "\u2014")
    compare(Core.chipPercentText(p, "used"), "\u2014")
  }

  function test_used_versus_remaining_metric() {
    var p = makeProvider("claude", "ready", 42, 58)
    compare(Core.chipPercentText(p, "remaining"), "58%")
    compare(Core.chipPercentText(p, "used"), "42%")
    compare(Core.displayMetric({ display: { metric: "used" } }), "used")
    compare(Core.displayMetric({ display: { metric: "remaining" } }), "remaining")
    compare(Core.displayMetric(null), "remaining")
  }

  // Live Quattro: snapshot windows arrive as array-like QVariantList where
  // Array.isArray is false but .length / [0] still work (chips stuck on "—").
  function test_array_like_windows_render_percent() {
    var p = {
      id: "amp",
      name: "Amp",
      state: "ready",
      windows: {
        length: 1,
        0: { id: "daily", label: "Daily", usedPercent: 0, remainingPercent: 100 }
      }
    }
    verify(!Array.isArray(p.windows))
    compare(Core.chipPercentText(p, "remaining"), "100%")
    compare(Core.chipPercentText(p, "used"), "0%")
    var lines = Core.windowDisplayLines(p, "remaining")
    compare(lines.length, 1)
    compare(lines[0].percentText, "100%")
  }

  function test_state_cues_for_stale_error_loading() {
    compare(Core.chipStateCue(makeProvider("claude", "ready", 1, 99)), "")
    compare(Core.chipStateCue(makeProvider("claude", "stale", 1, 99)), " ⌛")
    compare(Core.chipStateCue(makeProvider("claude", "cli_missing")), " !")
    compare(Core.chipStateCue(makeProvider("claude", "network_error")), " !")
    compare(Core.chipStateCue(makeProvider("claude", "loading")), "\u2026")
    verify(Core.chipDimmed(makeProvider("claude", "stale", 1, 99)))
    verify(!Core.chipDimmed(makeProvider("claude", "ready", 1, 99)))
  }

  function test_tooltip_includes_provider_percent_state_reset() {
    var p = makeProvider("claude", "ready", 42, 58, "2026-07-26T22:00:00Z")
    var tip = Core.chipTooltip(p, "remaining", Date.parse("2026-07-26T20:00:00Z"))
    verify(tip.indexOf("Claude") >= 0)
    verify(tip.indexOf("58%") >= 0)
    verify(tip.indexOf("ready") >= 0)
    verify(tip.indexOf("resets") >= 0)
    verify(tip.indexOf("2h 0m") >= 0)
    verify(tip.indexOf("2026-07-26T22:00:00Z") === -1)
  }

  // ---- Task 10: click routing ----

  function test_left_click_opens_provider() {
    var owner = {}
    var route = Core.routeChipClick("left", owner, "claude", null)
    compare(route.action, "requestPopup")
    compare(route.providerId, "claude")
    compare(route.view, "usage")
  }

  function test_left_click_same_provider_toggles_close() {
    var owner = {}
    var open = { owner: owner, providerId: "claude", view: "usage" }
    var route = Core.routeChipClick(1, owner, "claude", open)
    compare(route.action, "closePopup")
  }

  function test_left_click_other_provider_switches() {
    var owner = {}
    var open = { owner: owner, providerId: "claude", view: "usage" }
    var route = Core.routeChipClick("left", owner, "codex", open)
    compare(route.action, "requestPopup")
    compare(route.providerId, "codex")
  }

  function test_middle_click_refresh_all() {
    var route = Core.routeChipClick("middle", {}, "claude", null)
    compare(route.action, "refreshAll")
    compare(route.force, true)
    route = Core.routeChipClick(4, {}, "claude", null)
    compare(route.action, "refreshAll")
  }

  function test_right_click_opens_settings() {
    var owner = { id: "bar-1" }
    var route = Core.routeChipClick("right", owner, "claude", null)
    compare(route.action, "openSettings")
    compare(route.owner, owner)
    route = Core.routeChipClick(2, owner, "claude", null)
    compare(route.action, "openSettings")
  }

  function test_unknown_button_is_noop() {
    var route = Core.routeChipClick(8, {}, "claude", null)
    compare(route.action, "noop")
  }

  // ---- Task 10: registration + source guards ----

  function test_provider_chip_registers_and_unregisters() {
    fakeBar.clickTargets = []
    var chip = providerChipComp.createObject(testCase, {
      bar: fakeBar,
      providerId: "claude",
      displayName: "Claude",
      percentText: "90%",
      tooltipText: "Claude · 90% · ready"
    })
    verify(chip !== null)
    // Allow Component.onCompleted to run.
    wait(0)
    verify(fakeBar.clickTargets.indexOf(chip) >= 0)
    verify(typeof chip.triggerPress === "function")

    chip.destroy()
    wait(0)
    verify(fakeBar.clickTargets.indexOf(chip) < 0)
  }

  function test_provider_chip_trigger_press_emits_pressed() {
    var chip = providerChipComp.createObject(testCase, {
      bar: fakeBar,
      providerId: "codex",
      percentText: "80%"
    })
    var seen = -1
    chip.pressed.connect(function (button) { seen = button })
    chip.triggerPress(Qt.LeftButton)
    compare(seen, Qt.LeftButton)
    chip.destroy()
  }

  function test_source_guard_no_process_timer_shell() {
    var files = [widgetUrl, chipUrl]
    for (var i = 0; i < files.length; i++) {
      var xhr = new XMLHttpRequest()
      xhr.open("GET", files[i], false)
      xhr.send()
      var src = String(xhr.responseText)
      // Match type usage, not prose (rg -n 'Process|Timer|...' in the plan).
      verify(!/\bProcess\b/.test(src) || src.indexOf("//") >= 0)
      // Strip line comments then re-check forbidden owners.
      var code = src.replace(/\/\/[^\n]*/g, "")
      verify(code.indexOf("Process") < 0, files[i] + " must not own Process")
      verify(code.indexOf("Timer") < 0, files[i] + " must not own Timer")
      verify(src.indexOf("bash -lc") < 0, files[i])
      verify(src.indexOf("sh -c") < 0, files[i])
    }
  }

  function test_source_has_click_protocol_no_wheel() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", chipUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("registerClickTarget") >= 0)
    verify(src.indexOf("unregisterClickTarget") >= 0)
    verify(src.indexOf("function triggerPress") >= 0)
    verify(src.indexOf("onWheel") < 0)

    xhr.open("GET", widgetUrl, false)
    xhr.send()
    src = String(xhr.responseText)
    verify(src.indexOf("ProviderChip") >= 0)
    verify(src.indexOf("refreshAll") >= 0)
    verify(src.indexOf("openSettings") >= 0)
    verify(src.indexOf("requestPopup") >= 0)
    // UX-021: Popup is a direct child (Loader+Component left KeyboardPanel
    // required props unset → no panel on chip click).
    verify(src.indexOf("sourceComponent") < 0)
    verify(src.indexOf("Popup {") >= 0)
    // UX-003: no product brand chip label
    verify(src.indexOf("\"AB\"") < 0)
    verify(src.indexOf("Agent Bar") < 0)
  }

  function test_icon_files_exist_with_approved_names() {
    var names = ["claude.png", "codex.png", "amp.svg", "grok.svg"]
    for (var i = 0; i < names.length; i++) {
      var path = "file://" + repoRoot + "/assets/omarchy/icons/" + names[i]
      var xhr = new XMLHttpRequest()
      xhr.open("GET", path, false)
      xhr.send()
      // status 0 is common for local file:// success
      verify(xhr.status === 200 || xhr.status === 0, names[i] + " missing")
      verify(String(xhr.responseText || xhr.response).length > 0, names[i] + " empty")
    }
    compare(Core.iconFileName("claude"), "claude.png")
    compare(Core.iconFileName("codex"), "codex.png")
    compare(Core.iconFileName("amp"), "amp.svg")
    compare(Core.iconFileName("grok"), "grok.svg")
  }

  Component {
    id: agentChipComp
    AgentChip {}
  }

  Component {
    id: providerChipComp
    ProviderChipHost {}
  }

  // Inline host mirrors ProviderChip.qml without loading relative file URL issues.
  component ProviderChipHost: Item {
    id: chipRoot
    property var bar: null
    property string providerId: ""
    property string displayName: ""
    property string percentText: "\u2014"
    property string stateCue: ""
    property string tooltipText: ""
    property var registeredBar: null

    signal pressed(int button)

    function triggerPress(button) {
      if (chipRoot.bar && typeof chipRoot.bar.hideTooltip === "function")
        chipRoot.bar.hideTooltip(chipRoot)
      chipRoot.pressed(button)
    }

    function syncClickRegistration() {
      if (registeredBar && typeof registeredBar.unregisterClickTarget === "function")
        registeredBar.unregisterClickTarget(chipRoot)
      registeredBar = chipRoot.bar
      if (registeredBar && typeof registeredBar.registerClickTarget === "function")
        registeredBar.registerClickTarget(chipRoot)
    }

    onBarChanged: syncClickRegistration()
    Component.onCompleted: syncClickRegistration()
    Component.onDestruction: {
      if (registeredBar && typeof registeredBar.unregisterClickTarget === "function")
        registeredBar.unregisterClickTarget(chipRoot)
    }
  }
}
