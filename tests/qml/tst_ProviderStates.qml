import QtQuick
import QtTest
import "../../assets/omarchy/CoreView.js" as Core

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

  function read(rel) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/" + rel, false)
    xhr.send()
    return String(xhr.responseText || "")
  }

  function test_ready_windows_mode() {
    var p = firstProvider("ready.json")
    compare(Core.contentMode(p), "windows")
    verify(Core.planBadge(p).length > 0)
    var lines = Core.windowDisplayLines(p, "remaining")
    verify(lines.length > 0)
    verify(lines[0].percentText.indexOf("%") >= 0)
    verify(lines[0].percent >= 0 && lines[0].percent <= 100)
  }

  function test_empty_windows_ready_message() {
    var p = firstProvider("valid-empty-windows.json")
    compare(Core.contentMode(p), "empty_windows")
    compare(Core.stateBody(p), Core.emptyWindowsMessage())
    verify(Core.stateBody(p).indexOf("billed another way") >= 0)
  }

  function test_stale_retains_windows_and_label() {
    var p = firstProvider("valid-stale.json")
    compare(Core.contentMode(p), "stale_windows")
    compare(Core.stateTitle(p), "")
    compare(Core.stateBody(p), "")
    verify(Core.errorMessage(p).length > 0)
    var acts = Core.stateActions(p)
    var kinds = acts.map(function (a) { return a.kind })
    verify(kinds.indexOf("retry") >= 0)
    var lines = Core.windowDisplayLines(p, "used")
    compare(lines.length, 1)
    compare(lines[0].percentText, "90%")
    compare(lines[0].percent, 90)
  }

  function test_cli_missing_view_installation_and_check_again() {
    var p = firstProvider("valid-cli-missing.json")
    compare(Core.contentMode(p), "state")
    var acts = Core.stateActions(p)
    var kinds = acts.map(function (a) { return a.kind })
    verify(kinds.indexOf("view_installation") >= 0)
    // Fixture's error is retryable: false — "Check again" (a Retry action)
    // must not be offered for a non-retryable error (JSON-025 addendum).
    verify(kinds.indexOf("retry") < 0)
  }

  function test_unauthenticated_connect_or_install() {
    var p = firstProvider("valid-unauthenticated.json")
    compare(Core.contentMode(p), "state")
    var acts = Core.stateActions(p)
    verify(acts.length >= 1)
    var kind = acts[0].kind
    verify(kind === "login" || kind === "view_installation")
    // Label should be user-facing Sign in when login
    if (kind === "login")
      verify(acts[0].label.length > 0)
  }

  function test_network_and_rate_limit_are_retryable_not_auth() {
    var net = firstProvider("valid-network-error.json")
    var rate = firstProvider("valid-rate-limited.json")
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
    verify(h.connection === undefined)
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

  function readyWith(windows) {
    return { id: "claude", name: "Claude", state: "ready", windows: windows }
  }

  function layoutOf(windows, nowIso) {
    return Core.windowLayout(readyWith(windows), "remaining", Date.parse(nowIso))
  }

  function test_severity_level_thresholds() {
    compare(Core.severityLevel(89.9), "")
    compare(Core.severityLevel(90), "warning")
    compare(Core.severityLevel(94.9), "warning")
    compare(Core.severityLevel(95), "critical")
    compare(Core.severityLevel(100), "critical")
    compare(Core.severityLevel("nope"), "")
  }

  // The Warning level renders as "Low" (visual design §7); "warning" is the
  // internal name it shares with Rust.
  function test_severity_tag_words() {
    compare(Core.severityTagText("critical"), "Critical")
    compare(Core.severityTagText("warning"), "Low")
    compare(Core.severityTagText(""), "")
  }

  function test_provider_severity_is_the_worst_window() {
    compare(Core.providerSeverity(null), "")
    compare(Core.providerSeverity({ windows: [] }), "")
    compare(Core.providerSeverity({ windows: [{ usedPercent: 10 }, { usedPercent: 92 }] }),
            "warning")
    compare(Core.providerSeverity({ windows: [{ usedPercent: 92 }, { usedPercent: 97 }] }),
            "critical")
  }

  // §7: severity is computed from usedPercent, so switching the displayed
  // metric never changes what counts as critical.
  function test_severity_ignores_the_display_metric() {
    var provider = readyWith([{ id: "s", label: "S", usedPercent: 96, remainingPercent: 4,
                                resetsAt: "2026-07-28T18:00:00Z" }])
    var now = Date.parse("2026-07-28T15:00:00Z")
    compare(Core.windowLayout(provider, "remaining", now).lead.severity, "critical")
    compare(Core.windowLayout(provider, "used", now).lead.severity, "critical")
  }

  function test_lead_election_critical_beats_nearest_reset() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T15:30:00Z" },
      { id: "weekly", label: "Weekly (7d)", usedPercent: 96, remainingPercent: 4,
        resetsAt: "2026-07-31T11:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "weekly")
    compare(layout.lead.severity, "critical")
    compare(layout.rest.length, 1)
    compare(layout.rest[0].id, "session")
  }

  function test_lead_election_lowest_remaining_among_criticals() {
    var layout = layoutOf([
      { id: "a", label: "A", usedPercent: 96, remainingPercent: 4,
        resetsAt: "2026-07-28T15:30:00Z" },
      { id: "b", label: "B", usedPercent: 99, remainingPercent: 1,
        resetsAt: "2026-07-31T11:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "b")
  }

  function test_lead_election_nearest_future_reset_when_healthy() {
    var layout = layoutOf([
      { id: "weekly", label: "Weekly (7d)", usedPercent: 40, remainingPercent: 60,
        resetsAt: "2026-07-31T11:59:59Z" },
      { id: "session", label: "Session (5h)", usedPercent: 4, remainingPercent: 96,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "session")
    // The rest keeps delivered order, not election order.
    compare(layout.rest.length, 1)
    compare(layout.rest[0].id, "weekly")
  }

  function test_lead_election_ignores_elapsed_and_missing_resets() {
    var layout = layoutOf([
      { id: "past", label: "Past", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T14:00:00Z" },
      { id: "none", label: "None", usedPercent: 20, remainingPercent: 80, resetsAt: null },
      { id: "future", label: "Future", usedPercent: 30, remainingPercent: 70,
        resetsAt: "2026-07-29T09:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "future")
    compare(layout.rest.length, 2)
    compare(layout.rest[0].id, "past")
  }

  function test_lead_election_without_any_reset_takes_first_delivered() {
    var layout = layoutOf([
      { id: "first", label: "First", usedPercent: 20, remainingPercent: 80, resetsAt: null },
      { id: "second", label: "Second", usedPercent: 10, remainingPercent: 90, resetsAt: null }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "first")
    compare(layout.rest.length, 1)
  }

  function test_lead_election_ties_keep_delivered_order() {
    var layout = layoutOf([
      { id: "first", label: "First", usedPercent: 50, remainingPercent: 50,
        resetsAt: "2026-07-28T18:00:00Z" },
      { id: "second", label: "Second", usedPercent: 50, remainingPercent: 50,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "first")
  }

  function test_single_window_leads_with_no_rest() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 42, remainingPercent: 58,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "session")
    compare(layout.rest.length, 0)
  }

  function test_no_windows_means_no_lead() {
    var layout = layoutOf([], "2026-07-28T15:00:00Z")
    compare(layout.lead, null)
    compare(layout.rest.length, 0)
  }

  // A window id outside the old allowlist is now electable — the exact bug
  // the allowlist caused (it silently demoted anything unknown).
  function test_unknown_window_id_can_lead() {
    var layout = layoutOf([
      { id: "weekly-model:opus", label: "Opus", usedPercent: 97, remainingPercent: 3,
        resetsAt: "2026-07-31T11:59:59Z" },
      { id: "session", label: "Session (5h)", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T15:30:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "weekly-model:opus")
  }

  function test_lines_carry_raw_percentages_and_countdown() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 31, remainingPercent: 69,
        resetsAt: "2026-07-28T17:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.usedPercent, 31)
    compare(layout.lead.remainingPercent, 69)
    compare(layout.lead.percentText, "69%")
    compare(layout.lead.resetCountdown, "2h 59m")
    compare(layout.lead.resetPhrase, "resets in")
  }

  function test_chip_state_cue_stale_is_clock_glyph() {
    compare(Core.chipStateCue({ state: "stale" }), "󰅐")
  }

  function test_state_actions_respect_retryable() {
    var nonRetryable = {
      id: "claude", name: "Claude", state: "provider_error",
      error: { code: "provider_error", message: "x", retryable: false },
      action: { kind: "retry", label: "Retry", target: null }
    }
    var acts = Core.stateActions(nonRetryable)
    var hasRetry = false
    for (var i = 0; i < acts.length; i++)
      if (acts[i].kind === "retry") hasRetry = true
    verify(!hasRetry, "non-retryable error must not offer Retry")

    var retryable = {
      id: "claude", name: "Claude", state: "network_error",
      error: { code: "network_error", message: "x", retryable: true },
      action: { kind: "retry", label: "Retry", target: null }
    }
    acts = Core.stateActions(retryable)
    hasRetry = false
    for (i = 0; i < acts.length; i++)
      if (acts[i].kind === "retry") hasRetry = true
    verify(hasRetry, "retryable error must offer Retry")
  }

  function test_state_copy_cli_missing() {
    var p = { id: "grok", name: "Grok", state: "cli_missing", windows: [], error: null }
    compare(Core.stateTitle(p), "Grok CLI is not installed")
    compare(Core.stateBody(p), "Agent Bar reads the quota through it.")
  }

  function test_state_copy_unauthenticated() {
    var p = { id: "claude", name: "Claude", state: "unauthenticated", windows: [], error: null }
    compare(Core.stateTitle(p), "Not signed in to Claude")
    compare(Core.stateBody(p), "Signing in opens the official Claude CLI.")
  }

  function test_state_copy_rate_limited() {
    var p = { id: "codex", name: "Codex", state: "rate_limited", windows: [], error: null }
    compare(Core.stateTitle(p), "Codex hit a rate limit")
    compare(Core.stateBody(p), "Try again in a few minutes.")
  }

  function test_state_copy_network_error() {
    var p = { id: "amp", name: "Amp", state: "network_error", windows: [], error: null }
    compare(Core.stateTitle(p), "Cannot reach Amp")
    compare(Core.stateBody(p), "Check your connection.")
  }

  function test_state_copy_provider_error_and_unknown() {
    var pe = { id: "amp", name: "Amp", state: "provider_error", windows: [], error: null }
    compare(Core.stateTitle(pe), "Amp returned no limits")
    compare(Core.stateBody(pe), "")
    var unk = { id: "claude", name: "Claude", state: "weird", windows: [], error: null }
    compare(Core.stateTitle(unk), "Claude state is unknown")
  }

  function test_state_copy_ready_no_quota() {
    var p = { id: "claude", name: "Claude", state: "ready", windows: [], error: null }
    compare(Core.stateTitle(p), "Claude reports no quota")
    compare(Core.stateBody(p), "This account is billed another way.")
  }

  function test_state_body_error_message_precedence() {
    var p = { id: "grok", name: "Grok", state: "network_error", windows: [],
              error: { message: "DNS lookup failed.", retryable: true } }
    compare(Core.stateBody(p), "DNS lookup failed.")
  }

  function test_action_labels_renamed() {
    var auth = { id: "claude", name: "Claude", state: "unauthenticated", windows: [], error: null, action: null }
    var labels = Core.stateActions(auth).map(function (a) { return a.label })
    verify(labels.indexOf("Sign in") >= 0)
    verify(labels.indexOf("Connect") < 0)
    var cli = { id: "grok", name: "Grok", state: "cli_missing", windows: [], error: null,
                action: { kind: "view_installation", label: "", target: "https://example.com" } }
    var cliLabels = Core.stateActions(cli).map(function (a) { return a.label })
    verify(cliLabels.indexOf("Install guide") >= 0)
    verify(cliLabels.indexOf("Check again") >= 0)
  }

  // ProviderView.qml transitively imports qs.Commons/qs.Ui, which only
  // Quickshell's own runtime resolves — the bare Qt6 qmltestrunner used by
  // this gate cannot compile any file with `import qs.Commons` at all
  // (verified independently: even a one-line file with just that import
  // fails with the identical "module qs.Commons is not installed" error;
  // Task 5 already documented that no test in this repo live-instantiates
  // a qs.*-dependent component — see task-5-report.md). So the fix is
  // verified by source inspection instead of Qt.createComponent/createObject.
  function test_provider_view_timer_gates_on_active() {
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("property bool active: true") >= 0,
        "ProviderView must expose an owner-driven active prop")
    verify(view.indexOf("running: root.active") >= 0,
        "Timer must gate on active, not a child's visible prop")
    verify(view.indexOf("running: root.visible") < 0,
        "the fixed defect (visible-gated timer) must not resurface")
    verify(view.indexOf("nowTickRunning") >= 0,
        "a test-observable alias for the timer's running state must exist")
    verify(view.indexOf("onActiveChanged") >= 0,
        "reopening the popup must refresh nowMs, not show a stale countdown")

    var popup = read("assets/omarchy/Popup.qml")
    verify(popup.indexOf("active: root.isOpen") >= 0,
        "Popup must drive ProviderView.active from its own open state")
  }
}
