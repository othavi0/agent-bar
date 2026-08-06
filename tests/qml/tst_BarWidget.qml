import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core
import "../../assets/omarchy/CoreService.js" as Kernel

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
  property string coreViewUrl: "file://" + repoRoot + "/assets/omarchy/CoreView.js"

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

  function test_chip_dimmed_reflects_ready_state() {
    verify(Core.chipDimmed(makeProvider("claude", "stale", 1, 99)))
    verify(!Core.chipDimmed(makeProvider("claude", "ready", 1, 99)))
  }

  function test_chip_state_cue() {
    compare(Core.chipStateCue(null), "")
    compare(Core.chipStateCue({ state: "ready" }), "")
    compare(Core.chipStateCue({ state: "loading" }), "")
    compare(Core.chipStateCue({ state: "stale" }), "󰅐")
    compare(Core.chipStateCue({ state: "cli_missing" }), "!")
    compare(Core.chipStateCue({ state: "unauthenticated" }), "!")
    compare(Core.chipStateCue({ state: "rate_limited" }), "!")
    compare(Core.chipStateCue({ state: "network_error" }), "!")
    compare(Core.chipStateCue({ state: "provider_error" }), "!")
    // §7: a ready provider over the critical threshold earns the same cue.
    compare(Core.chipStateCue({ state: "ready", windows: [{ usedPercent: 96 }] }), "!")
    compare(Core.chipStateCue({ state: "ready", windows: [{ usedPercent: 92 }] }), "")
    // A state cue outranks severity: the clock keeps the stale meaning.
    compare(Core.chipStateCue({ state: "stale", windows: [{ usedPercent: 96 }] }), "󰅐")
  }

  // The urgent tint belongs to severity, never to the error cue — the
  // approved mockup shows critical Claude urgent and disconnected Grok plain.
  function test_chip_severity_urgent_only_when_ready_and_critical() {
    compare(Core.chipSeverityUrgent(null), false)
    compare(Core.chipSeverityUrgent({ state: "ready", windows: [{ usedPercent: 96 }] }), true)
    compare(Core.chipSeverityUrgent({ state: "ready", windows: [{ usedPercent: 92 }] }), false)
    compare(Core.chipSeverityUrgent({ state: "stale", windows: [{ usedPercent: 96 }] }), false)
    compare(Core.chipSeverityUrgent({ state: "network_error", windows: [] }), false)
  }

  // Plan 02 deferred minor: the cue used to expose its raw glyph.
  function test_chip_cue_label_is_a_word() {
    compare(Core.chipCueLabel({ state: "ready", windows: [{ usedPercent: 96 }] }), "critical")
    compare(Core.chipCueLabel({ state: "stale", windows: [] }), "stale")
    compare(Core.chipCueLabel({ state: "cli_missing", windows: [] }), "no CLI")
    compare(Core.chipCueLabel({ state: "unauthenticated", windows: [] }), "signed out")
    compare(Core.chipCueLabel({ state: "ready", windows: [{ usedPercent: 10 }] }), "")
  }

  function sourceAt(url) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", url, false)
    xhr.send()
    return String(xhr.responseText)
  }

  function test_chip_source_binds_severity() {
    var chip = sourceAt(chipUrl)
    verify(chip.indexOf("property bool severityUrgent") >= 0)
    verify(chip.indexOf("property string cueLabel") >= 0)
    verify(chip.indexOf("Color.urgent") >= 0,
           "severity uses the host urgent token, never a literal")
    verify(chip.indexOf("Accessible.name: root.stateCue") < 0,
           "the cue must speak a word, not its glyph")
    var widget = sourceAt(widgetUrl)
    verify(widget.indexOf("chipSeverityUrgent") >= 0)
    verify(widget.indexOf("chipCueLabel") >= 0)
  }

  function test_chip_accessible_label_humanized() {
    var ready = { name: "Claude", state: "ready",
                  windows: [{ usedPercent: 4, remainingPercent: 96 }] }
    compare(Core.chipAccessibleLabel(ready, "remaining"), "Claude · 96%")

    var signedOut = { name: "Claude", state: "unauthenticated", windows: [] }
    compare(Core.chipAccessibleLabel(signedOut, "remaining"),
            "Claude · signed out")

    var rateLimited = { name: "Codex", state: "rate_limited",
                        windows: [{ usedPercent: 98, remainingPercent: 2 }] }
    compare(Core.chipAccessibleLabel(rateLimited, "used"),
            "Codex · 98% · rate limited")

    var noCli = { name: "Grok", state: "cli_missing", windows: [] }
    compare(Core.chipAccessibleLabel(noCli, "remaining"), "Grok · no CLI")

    var failed = { name: "Amp", state: "provider_error", windows: [] }
    compare(Core.chipAccessibleLabel(failed, "remaining"), "Amp · failed")

    var emptyReady = { name: "Claude", state: "ready", windows: [] }
    compare(Core.chipAccessibleLabel(emptyReady, "remaining"), "Claude · —")

    var loading = { name: "Claude", state: "loading", windows: [] }
    compare(Core.chipAccessibleLabel(loading, "remaining"), "Claude · loading")

    var stale = { name: "Claude", state: "stale",
                  windows: [{ usedPercent: 95, remainingPercent: 5 }] }
    compare(Core.chipAccessibleLabel(stale, "remaining"), "Claude · 5% · stale")

    // Single-line by construction; no provider stays empty.
    compare(Core.chipAccessibleLabel(null, "remaining"), "")
  }

  function test_state_qualifier_strings() {
    compare(Core.stateQualifier("ready"), "")
    compare(Core.stateQualifier("stale"), "stale")
    compare(Core.stateQualifier("loading"), "loading")
    compare(Core.stateQualifier("cli_missing"), "no CLI")
    compare(Core.stateQualifier("unauthenticated"), "signed out")
    compare(Core.stateQualifier("rate_limited"), "rate limited")
    compare(Core.stateQualifier("network_error"), "offline")
    compare(Core.stateQualifier("provider_error"), "failed")
    compare(Core.stateQualifier("bogus"), "unknown")
    compare(Core.stateQualifier(""), "unknown")
  }

  function test_chip_numeral_text() {
    compare(Core.chipNumeralText({ state: "loading", windows: [] }, "remaining"), "···")
    compare(Core.chipNumeralText({ state: "ready", windows: [] }, "remaining"), "—")
    var ready = { state: "ready", windows: [{ usedPercent: 4, remainingPercent: 96 }] }
    compare(Core.chipNumeralText(ready, "remaining"), "96%")
    compare(Core.chipNumeralText(ready, "used"), "4%")
    compare(Core.chipNumeralText(null, "remaining"), "—")
  }

  function test_icon_optical_scale_covers_catalog() {
    var ids = Object.keys(Kernel.CLOSED_PROVIDERS)
    verify(ids.length >= 4)
    for (var i = 0; i < ids.length; i++) {
      var s = Core.iconOpticalScale(ids[i])
      verify(isFinite(s) && s > 0 && s <= 1)
    }
    compare(Core.iconOpticalScale("grok"), 0.875)
    compare(Core.iconOpticalScale("claude"), 1.0)
    compare(Core.iconOpticalScale("codex"), 1.0)
    compare(Core.iconOpticalScale("amp"), 1.0)
  }

  function test_icon_tinted_monochrome_marks_only() {
    verify(Core.iconTinted("codex"))
    verify(Core.iconTinted("grok"))
    verify(!Core.iconTinted("claude"))
    verify(!Core.iconTinted("amp"))
    verify(!Core.iconTinted(""))
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
      numeralText: "90%",
      accessibleLabel: "Claude · 90% · ready"
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
      numeralText: "80%"
    })
    var seen = -1
    chip.pressed.connect(function (button) { seen = button })
    chip.triggerPress(Qt.LeftButton)
    compare(seen, Qt.LeftButton)
    chip.destroy()
  }

  function test_bar_widget_renders_no_tooltip() {
    var widget = sourceAt(widgetUrl)
    verify(widget.indexOf("tooltipText") < 0,
           "the bar chip must never feed the host tooltip")
    verify(widget.indexOf("tooltipNowMs") < 0,
           "the tooltip clock died with the tooltip")
    verify(widget.indexOf("chipAccessibleLabel") >= 0,
           "the accessible label must still reach the chip")
    var chip = sourceAt(chipUrl)
    verify(chip.indexOf("property string accessibleLabel") >= 0)
    verify(chip.indexOf("Accessible.name: root.accessibleLabel") >= 0)
    verify(chip.indexOf("tooltipText") < 0,
           "the chip must not reference the host tooltip property")
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

  function test_source_chip_is_widgetbutton_no_wheel() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", chipUrl, false)
    xhr.send()
    var chip = String(xhr.responseText)
    // UX-010: the protocol is inherited from WidgetButton — exactly one
    // registration, owned by the host component. Our source must not add a
    // second protocol layer or a second mouse layer.
    verify(chip.indexOf("WidgetButton {") >= 0)
    verify(chip.indexOf("registerClickTarget") < 0)
    verify(chip.indexOf("MouseArea") < 0)
    // UX-009: wheel stays a no-op — no handler in our source.
    verify(chip.indexOf("onWheel") < 0)
    verify(chip.indexOf("wheelMoved") < 0)
    // A11Y-013: no plugin-authored motion (tst_Accessibility also guards).
    verify(chip.indexOf("Behavior") < 0)
    // §5 amended 2026-08-04 (owner picked it on live mockups): the numeral
    // box is tight — width follows the text, no reserved "100%" box. The
    // fixed box parked ~2 digits of slack in the inter-chip gap whenever
    // every numeral was short, which read as disproportionate spacing and
    // an inflated right edge. Chips may shift on digit-count changes; the
    // state cues (! / ln) already moved them, so nothing new is lost.
    verify(chip.indexOf("TextMetrics") < 0)
    verify(chip.indexOf('"100%"') < 0)
    verify(chip.indexOf("advanceWidth") < 0)
    verify(chip.indexOf("Text.AlignRight") < 0)
    verify(chip.indexOf("Style.bar.iconCanvas") >= 0)
    verify(chip.indexOf("MultiEffect") >= 0)
    verify(chip.indexOf("colorization") >= 0)
    verify(chip.indexOf("width: 13") < 0)
    verify(chip.indexOf("⌛") < 0)

    xhr.open("GET", widgetUrl, false)
    xhr.send()
    var widget = String(xhr.responseText)
    verify(widget.indexOf("ProviderChip") >= 0)
    verify(widget.indexOf("refreshAll") >= 0)
    verify(widget.indexOf("openSettings") >= 0)
    verify(widget.indexOf("requestPopup") >= 0)
    // UX-021: Popup is a direct child (Loader+Component left KeyboardPanel
    // required props unset → no panel on chip click).
    verify(widget.indexOf("sourceComponent") < 0)
    verify(widget.indexOf("Popup {") >= 0)
    // UX-003: no product brand chip label
    verify(widget.indexOf("\"AB\"") < 0)
    verify(widget.indexOf("Agent Bar") < 0)
    // Task 1 functions actually wired:
    verify(widget.indexOf("chipNumeralText") >= 0)
    verify(widget.indexOf("iconTinted") >= 0)
    verify(widget.indexOf("iconOpticalScale") >= 0)
    // WidgetButton's vertical/barSize are readonly; qmllint is verifiably
    // silent on assigning them (plan-02 finding) — failure would be runtime-only.
    verify(widget.indexOf("vertical: root.vertical") < 0)
    verify(widget.indexOf("barSize: root.barSize") < 0)
    verify(widget.indexOf("fontPixelSize:") < 0)
  }

  // Plan 03: the popup banner drops its ⌛ for 󰅐 (glyph parity with the chip
  // stale cue) — ban the emoji repo-wide across every file that could render
  // provider-facing copy, not just CoreView.js.
  function test_no_emoji_hourglass_in_assets() {
    var files = [
      "assets/omarchy/CoreView.js",
      "assets/omarchy/components/ProviderChip.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/ProviderRail.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/Popup.qml",
      "assets/omarchy/BarWidget.qml"
    ]
    for (var i = 0; i < files.length; i++) {
      var xhr = new XMLHttpRequest()
      xhr.open("GET", "file://" + repoRoot + "/" + files[i], false)
      xhr.send()
      var src = String(xhr.responseText)
      verify(src.indexOf("⌛") < 0, files[i] + " must not use the emoji hourglass")
    }
  }

  // §5 amended 2026-08-01 (owner picked it on live mockups): chips sit at
  // spacing.md (6). The old xxl (12) read as scattered once the numeral
  // moved beside the icon and its box slack joined the inter-chip gap.
  function test_chip_row_spacing_is_md() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", widgetUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("columnSpacing: Style.spacing.md") >= 0)
    verify(src.indexOf("columnSpacing: Style.spacing.xxl") < 0,
           "the scattered spacing must not come back")
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

  // Inline host mirrors WidgetButton's click-target/triggerPress contract
  // (registerClickTarget/unregisterClickTarget/triggerPress) plus
  // ProviderChip's own visual props, since the real ProviderChip.qml (built
  // on the host WidgetButton) cannot be instantiated here \u2014 qs.Commons/qs.Ui
  // are unresolvable in this pure Qt 6 runner. It exists to prove
  // BarWidget-side routing (refreshAll/openSettings/requestPopup), not the
  // chip's rendering.
  component ProviderChipHost: Item {
    id: chipRoot
    property var bar: null
    property string providerId: ""
    property string displayName: ""
    property string numeralText: "\u2014"
    property string stateCue: ""
    property string accessibleLabel: ""
    property var registeredBar: null

    width: 40
    height: 20

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
