import QtQuick
import QtTest
import "../../assets/omarchy/CoreSettings.js" as Core
import "../../assets/omarchy/CoreService.js" as Service
import "../../assets/omarchy/CoreView.js" as View

TestCase {
  id: testCase
  name: "AgentBarSettings"
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

  // ---- Pure draft / validation ----

  function test_default_settings_valid() {
    var d = Service.defaultSettings()
    var v = Core.validateSettingsDraft(d)
    compare(v.ok, true)
  }

  function test_provider_toggle_and_order() {
    var d = Service.defaultSettings()
    d = Core.setProviderEnabled(d, "codex", false)
    compare(d.providers[1].id, "codex")
    compare(d.providers[1].enabled, false)

    d = Core.moveProvider(d, "grok", -1) // amp, grok swap near end
    // default: claude, codex, amp, grok → move grok up → claude, codex, grok, amp
    compare(d.providers[2].id, "grok")
    compare(d.providers[3].id, "amp")

    d = Core.moveProvider(d, "claude", -1) // already top — no-op
    compare(d.providers[0].id, "claude")
  }

  function test_display_metric_and_interval_bounds() {
    var d = Service.defaultSettings()
    d = Core.setDisplayMetric(d, "used")
    compare(d.display.metric, "used")
    d = Core.setDisplayMetric(d, "remaining")
    compare(d.display.metric, "remaining")

    d = Core.setRefreshInterval(d, 30)
    compare(d.refreshIntervalSeconds, 30)
    d = Core.setRefreshInterval(d, 3600)
    compare(d.refreshIntervalSeconds, 3600)

    var bad = Core.setRefreshInterval(d, 10)
    compare(Core.validateSettingsDraft(bad).ok, false)
    bad = Core.setRefreshInterval(d, 9999)
    compare(Core.validateSettingsDraft(bad).ok, false)
  }

  function test_notifications_toggle() {
    var d = Service.defaultSettings()
    d = Core.setNotificationsEnabled(d, false)
    compare(d.notifications.enabled, false)
    d = Core.setNotificationsEnabled(d, true)
    compare(d.notifications.enabled, true)
  }

  function test_restore_defaults_draft_only() {
    var state = Core.settingsOpen(null, Service.defaultSettings(), 1)
    state = Core.settingsMarkDirty(state)
    state.draft = Core.setProviderEnabled(state.draft, "claude", false)
    state = Core.settingsRestoreDefaults(state)
    compare(state.phase, "dirty")
    compare(state.draft.providers[0].enabled, true)
    // Snapshot untouched
    compare(state.snapshot.providers[0].enabled, true)
  }

  function test_cancel_restores_snapshot() {
    var snap = Service.defaultSettings()
    var state = Core.settingsOpen(null, snap, 2)
    state.draft = Core.setDisplayMetric(state.draft, "used")
    state = Core.settingsMarkDirty(state)
    compare(state.phase, "dirty")
    state = Core.settingsCancel(state)
    compare(state.phase, "clean")
    compare(state.draft.display.metric, "remaining")
  }

  function test_invalid_save_disabled() {
    var state = Core.settingsOpen(null, Service.defaultSettings(), 3)
    compare(Core.settingsCanSave(state, state.draft), false) // clean
    state = Core.settingsMarkDirty(state)
    state.draft = Core.setRefreshInterval(state.draft, 5)
    compare(Core.settingsCanSave(state, state.draft), false) // invalid
    state.draft = Core.setRefreshInterval(state.draft, 60)
    compare(Core.settingsCanSave(state, state.draft), true)
  }

  function test_loading_locks_controls() {
    var state = Core.settingsBeginLoad(1)
    compare(state.phase, "loading")
    compare(Core.settingsControlsLocked(state), true)
    state = Core.settingsFinishLoad(state, 1, Service.defaultSettings())
    compare(state.phase, "clean")
    compare(Core.settingsControlsLocked(state), false)
  }

  function test_save_begin_captures_payload() {
    var state = Core.settingsOpen(null, Service.defaultSettings(), 5)
    state = Core.settingsMarkDirty(state)
    var payload = Core.cloneDraft(state.draft)
    payload = Core.setDisplayMetric(payload, "used")
    state = Core.settingsBeginSave(state, 6, payload)
    compare(state.phase, "saving")
    compare(state.generation, 6)
    compare(state.busy, true)
    compare(state.pendingPayload.display.metric, "used")
  }

  // ---- Source / UI contracts ----

  function test_settings_view_source_contracts() {
    var src = read("assets/omarchy/SettingsView.qml")
    verify(src.indexOf("Restore defaults") >= 0)
    verify(src.indexOf("Save changes") >= 0)
    verify(src.indexOf("Cancel") >= 0)
    verify(src.indexOf("NumberField") >= 0)
    verify(src.indexOf("Notifications") >= 0)
    verify(src.indexOf("Remaining") >= 0)
    verify(src.indexOf("Used") >= 0)
    verify(src.indexOf("MaintenanceView") >= 0)
    // No credentials / money / theme / cache editor
    verify(src.indexOf("credential") < 0 || src.toLowerCase().indexOf("no credential") >= 0)
    verify(src.indexOf("password") < 0)
    verify(src.indexOf("apiKey") < 0)
    verify(!View.containsMoneyCopy(src))
    verify(src.indexOf("Text.RichText") < 0)
    // Copy design §5.7. The old labels are banned by name so a revert fails
    // here rather than silently shipping.
    verify(src.indexOf("Bar shows") >= 0)
    verify(src.indexOf("Chip number") < 0)
    verify(src.indexOf("Refresh every") >= 0)
    verify(src.indexOf("Refresh interval (seconds)") < 0)
    verify(src.indexOf("Warn me before a quota runs out.") >= 0)
    verify(src.indexOf("Usage threshold alerts") < 0)
    verify(src.indexOf('text: "Loading\\u2026"') >= 0)
    verify(src.indexOf("Loading settings") < 0)
    // The host NumberField has no suffix property, so the unit is a sibling
    // label positioned against the spin box.
    verify(src.indexOf('text: "seconds"') >= 0)
  }

  function test_settings_row_has_icon_name_chevrons() {
    var src = read("assets/omarchy/components/SettingsProviderRow.qml")
    verify(src.indexOf("Image") >= 0)
    verify(src.indexOf("displayName") >= 0)
    verify(src.indexOf("󰅃") >= 0)
    verify(src.indexOf("󰅀") >= 0)
    verify(src.indexOf("enableToggled") >= 0)
  }

  function test_service_settings_argv_shapes() {
    var show = Core.settingsArgvShow("/tmp/agent-bar")
    compare(show[0], "/tmp/agent-bar")
    compare(show[1], "config")
    compare(show[2], "show")
    var apply = Core.settingsArgvApplyStdin("/tmp/agent-bar")
    compare(apply[3], "stdin")
  }

  function test_service_qml_has_settings_methods() {
    var src = read("assets/omarchy/Service.qml")
    verify(src.indexOf("function saveSettings") >= 0)
    verify(src.indexOf("function cancelSettings") >= 0)
    verify(src.indexOf("function restoreSettingsDefaults") >= 0)
    verify(src.indexOf("pendingSettingsPayload") >= 0)
    verify(src.indexOf("stdinEnabled: true") >= 0)
    verify(src.indexOf("config\", \"apply\", \"stdin\"") >= 0 || src.indexOf("settingsArgvApplyStdin") >= 0)
  }

  // Settings write mirrors the maintenance handoff EOF pattern: `config apply
  // stdin` reads until EOF, so write() must be followed by stdinEnabled=false
  // or the helper hangs forever and the save never lands.
  function test_service_settings_stdin_closes_after_write() {
    var src = read("assets/omarchy/Service.qml")
    var proc = src.indexOf("id: settingsWriteProcess")
    verify(proc >= 0)
    var onStarted = src.indexOf("onStarted:", proc)
    verify(onStarted >= 0)
    var onExited = src.indexOf("onExited:", onStarted)
    verify(onExited > onStarted)
    var body = src.substring(onStarted, onExited)
    verify(body.indexOf("write(") >= 0)
    var writeAt = body.indexOf("write(")
    var closeAt = body.indexOf("stdinEnabled = false", writeAt)
    verify(closeAt > writeAt, "stdinEnabled=false must follow write for EOF")
    // Re-arm before each start so consecutive saves keep a writable stdin.
    var kick = src.indexOf("function kickSettingsWrite")
    verify(kick >= 0)
    var kickEnd = src.indexOf("function applySettingsWriteResult", kick)
    verify(kickEnd > kick)
    var kickBody = src.substring(kick, kickEnd)
    verify(kickBody.indexOf("stdinEnabled = true") >= 0,
           "kickSettingsWrite must re-arm stdin before start")
  }

  function test_popup_hosts_settings_view() {
    var src = read("assets/omarchy/Popup.qml")
    verify(src.indexOf("SettingsView") >= 0)
    verify(src.indexOf("settingsStub") < 0)
  }
}
